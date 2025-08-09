use crate::tui::mainbound_message::MainboundMessage;
use mft::MftEntry;
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

    process_mft_bytes(index, mft_bytes, tx.clone())?;

    tx.send(MainboundMessage::Complete { file_index: index })?;
    Ok(())
}

pub fn process_mft_bytes(
    index: usize,
    mft_bytes: Vec<u8>,
    tx: std::sync::mpsc::Sender<MainboundMessage>,
) -> eyre::Result<()> {
    let mut parser = MftParser::from_buffer(mft_bytes)
        .map_err(|e| eyre::eyre!("Failed to parse MFT bytes: {}", e))?;
    let entry_size = Information::new::<byte>(parser.entry_size as f64);

    tx.send(MainboundMessage::EntrySizeDiscovered {
        file_index: index,
        entry_size,
    })?;

    // Maintain directory map: record_number -> (name, parent_record_number)
    let mut directories: HashMap<u64, (String, Option<u64>)> = HashMap::new();

    for entry in parser.iter_entries() {
        let messages = process_mft_entry(index, entry_size, entry, &mut directories);
        for message in messages {
            tx.send(message)
                .map_err(|e| eyre::eyre!("Failed to send message: {}", e))?;
        }
    }
    Ok(())
}

fn reconstruct_full_path(record_number: u64, directories: &HashMap<u64, (String, Option<u64>)>) -> Option<String> {
    let mut components = Vec::new();
    let mut current = Some(record_number);
    let mut depth = 0usize;
    while let Some(id) = current {
        if depth > 1024 { break; } // safety guard
        if let Some((name, parent)) = directories.get(&id) {
            if name == "." || id == 5 { // stop at root like original logic
                break;
            }
            components.push(name.clone());
            current = *parent;
        } else {
            break;
        }
        depth += 1;
    }
    if components.is_empty() { return None; }
    components.reverse();
    Some(format!("\\{}", components.join("\\")))
}

pub fn process_mft_entry(
    index: usize,
    entry_size: Information,
    entry: mft::err::Result<MftEntry>,
    directories: &mut HashMap<u64, (String, Option<u64>)>,
) -> Vec<MainboundMessage> {
    let mut rtn = Vec::new();
    match entry {
        Ok(entry) => {
            let record_number = entry.header.record_number;
            // Track if we discovered at least one valid filename (avoid duplicates)
            let mut discovered_paths: Vec<std::path::PathBuf> = Vec::new();
            // Iterate attributes once, collect names + parent ref, also standard health status
            for attribute in entry.iter_attributes() {
                let Ok(attribute) = attribute else { continue; };
                if let MftAttributeContent::AttrX30(filename_attribute) = &attribute.data {
                    let filename = &filename_attribute.name;
                    if filename.is_empty()
                        || filename.starts_with('$')
                        || filename == "."
                        || filename == ".."
                        || filename.len() <= 2
                    {
                        continue;
                    }
                    let parent_ref = if filename_attribute.parent.entry == 0 {
                        None
                    } else {
                        Some(filename_attribute.parent.entry)
                    };
                    // Insert if not already present to preserve first seen (preferred) name
                    directories.entry(record_number).or_insert((filename.clone(), parent_ref));
                    if let Some(full_path) = reconstruct_full_path(record_number, directories) {
                        discovered_paths.push(std::path::PathBuf::from(full_path));
                    }
                }
            }
            // Always push entry status (healthy)
            rtn.push(MainboundMessage::EntryStatus { file_index: index, is_healthy: true });
            // Emit discovered files (may be empty)
            if !discovered_paths.is_empty() {
                rtn.push(MainboundMessage::DiscoveredFiles { file_index: index, files: discovered_paths });
            }
            // Progress message
            rtn.push(MainboundMessage::Progress { file_index: index, processed_size: entry_size });
        }
        Err(e) => {
            rtn.push(MainboundMessage::EntryStatus { file_index: index, is_healthy: false });
            rtn.push(MainboundMessage::Progress { file_index: index, processed_size: entry_size });
            rtn.push(MainboundMessage::Error { file_index: index, error: Line::from(format!("Error processing entry: {e}")) });
            return rtn;
        }
    }
    rtn
}
