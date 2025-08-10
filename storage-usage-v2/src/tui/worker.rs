use crate::tui::mainbound_message::MainboundMessage;
use mft::MftParser;
use mft::attribute::MftAttributeContent;
use ratatui::text::Line;
use rayon::iter::IndexedParallelIterator;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;
use uom::si::f64::Information;
use uom::si::information::byte;

// Promote DirectoryEntry so helper can see it
#[derive(Clone)]
struct DirectoryEntry { name: String, parent: Option<u64> }

pub fn start_workers(
    mft_files: Vec<PathBuf>,
) -> eyre::Result<(Receiver<MainboundMessage>, JoinHandle<eyre::Result<()>>)> {
    let (tx, rx) = std::sync::mpsc::channel::<MainboundMessage>();
    let handle = std::thread::spawn(move || {
        // disabled for now
        const DISABLE_WORKER: bool = false;
        if !DISABLE_WORKER {
            mft_files
                .into_par_iter()
                .enumerate()
                .try_for_each(|(index, mft_file)| process_mft_file(index, mft_file, tx.clone()))?;
        }
        Ok(())
    });
    Ok((rx, handle))
}

pub fn process_mft_file(
    index: usize,
    mft_file: PathBuf,
    tx: std::sync::mpsc::Sender<MainboundMessage>,
) -> eyre::Result<()> {
    // read from os
    let file_size_bytes = std::fs::metadata(&mft_file)
        .map_err(|e| eyre::eyre!("Failed to read metadata for {}: {}", mft_file.display(), e))?
        .len();
    tx.send(MainboundMessage::FileSizeDiscovered {
        file_index: index,
        file_size: Information::new::<byte>(file_size_bytes as f64),
    })?;

    // Infer drive letter from file stem (e.g. C.mft -> 'C')
    let drive_letter = mft_file
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.chars().next())
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('?');

    // Memory map the file
    let file = std::fs::File::open(&mft_file)
        .map_err(|e| eyre::eyre!("Failed to open file {}: {}", mft_file.display(), e))?;
    let mmap = unsafe {
        memmap2::MmapOptions::new()
            .map(&file)
            .map_err(|e| eyre::eyre!("Failed to memory map file {}: {}", mft_file.display(), e))?
    };
    let mft_bytes = mmap.as_ref().to_vec();
    drop(mmap);

    process_mft_bytes(index, mft_bytes, drive_letter, tx.clone())?;

    tx.send(MainboundMessage::Complete { file_index: index })?;
    Ok(())
}

pub fn process_mft_bytes(
    index: usize,
    mft_bytes: Vec<u8>,
    drive_letter: char,
    tx: std::sync::mpsc::Sender<MainboundMessage>,
) -> eyre::Result<()> {
    let mut parser = MftParser::from_buffer(mft_bytes)
        .map_err(|e| eyre::eyre!("Failed to parse MFT bytes: {}", e))?;
    let entry_size = Information::new::<byte>(parser.entry_size as f64);

    tx.send(MainboundMessage::EntrySizeDiscovered {
        file_index: index,
        entry_size,
    })?;

    #[derive(Clone)]
    struct PendingEntry {
        record_number: u64,
        filename: String,
        parent_ref: Option<u64>,
    }

    let mut directories: HashMap<u64, DirectoryEntry> = HashMap::new();
    let mut pending: HashMap<u64, Vec<PendingEntry>> = HashMap::new();
    let mut resolve_queue: Vec<PendingEntry> = Vec::new();

    for entry in parser.iter_entries() {
        // progress & health first (assume healthy unless error below)
        let mut healthy = true;
        match &entry {
            Ok(_) => tx.send(MainboundMessage::EntryStatus { file_index: index, is_healthy: true })?,
            Err(_) => { healthy = false; tx.send(MainboundMessage::EntryStatus { file_index: index, is_healthy: false })?; }
        }

        let (record_number, attributes) = match entry {
            Ok(e) => (e.header.record_number, Some(e)),
            Err(e) => {
                tx.send(MainboundMessage::Error { file_index: index, error: Line::from(format!("Error processing entry: {e}")) })?;
                tx.send(MainboundMessage::Progress { file_index: index, processed_size: entry_size })?;
                continue;
            }
        };

        let mut discovered: Vec<PathBuf> = Vec::new();

        // Walk attributes, only use first filename (X30)
        if let Some(entry_ok) = attributes {
            for attribute in entry_ok.iter_attributes() {
                let Ok(attribute) = attribute else { continue; };
                if let MftAttributeContent::AttrX30(filename_attr) = &attribute.data {
                    let filename = &filename_attr.name;
                    if filename.is_empty() || filename.starts_with('$') || filename == "." || filename == ".." { continue; }
                    let parent_ref = if filename_attr.parent.entry == 0 { None } else { Some(filename_attr.parent.entry) };
                    // Insert directory (enables traversal); overwrite is fine (latest wins) but we could keep first
                    directories.insert(record_number, DirectoryEntry { name: filename.clone(), parent: parent_ref });
                    // Try immediate full path
                    match try_build_full_path(filename, parent_ref, &directories, drive_letter) {
                        Ok(full_path) => {
                            discovered.push(PathBuf::from(full_path));
                            // New directory may unblock children
                            if let Some(children) = pending.remove(&record_number) { resolve_queue.extend(children); }
                        }
                        Err(missing_parent) => {
                            pending.entry(missing_parent).or_default().push(PendingEntry { record_number, filename: filename.clone(), parent_ref });
                        }
                    }
                    // Resolve queue breadth-first
                    while let Some(pend) = resolve_queue.pop() {
                        match try_build_full_path(&pend.filename, pend.parent_ref, &directories, drive_letter) {
                            Ok(path) => {
                                discovered.push(PathBuf::from(path));
                                if let Some(children) = pending.remove(&pend.record_number) { resolve_queue.extend(children); }
                            }
                            Err(missing_parent) => {
                                pending.entry(missing_parent).or_default().push(pend);
                            }
                        }
                    }
                    break; // only first X30
                }
            }
        }

        if !discovered.is_empty() {
            tx.send(MainboundMessage::DiscoveredFiles { file_index: index, files: discovered })?;
        }
        // progress message
        tx.send(MainboundMessage::Progress { file_index: index, processed_size: entry_size })?;
        if !healthy { continue; }
    }

    // Flush unresolved pending entries with minimal fallback path
    for (_missing, entries) in pending.into_iter() {
        let mut batch: Vec<PathBuf> = Vec::new();
        for pend in entries {
            let partial = if drive_letter != '?' { format!("{drive_letter}:\\{}", pend.filename) } else { pend.filename };
            batch.push(PathBuf::from(partial));
        }
        if !batch.is_empty() { tx.send(MainboundMessage::DiscoveredFiles { file_index: index, files: batch })?; }
    }

    Ok(())
}

fn try_build_full_path(
    filename: &str,
    parent_ref: Option<u64>,
    directories: &HashMap<u64, DirectoryEntry>,
    drive_letter: char,
) -> Result<String, u64> {
    let mut components = vec![filename.to_string()];
    let mut current = parent_ref;
    let mut guard = 0usize;
    while let Some(pid) = current {
        if guard > 4096 { break; }
        if pid == 5 { break; } // root sentinel
        if let Some(dir) = directories.get(&pid) {
            if dir.name == "." { break; }
            components.push(dir.name.clone());
            current = dir.parent;
        } else {
            return Err(pid);
        }
        guard += 1;
    }
    components.reverse();
    if drive_letter == '?' { Ok(format!("\\{}", components.join("\\"))) } else { Ok(format!("{drive_letter}:\\{}", components.join("\\"))) }
}
