use crate::tui::mainbound_message::MainboundMessage;
use mft::MftEntry;
use mft::MftParser;
use mft::attribute::MftAttributeContent;
use ratatui::text::Line;
use rayon::iter::IndexedParallelIterator;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;
use uom::si::information::byte;
use uom::si::u64::Information;

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
        file_size: Information::new::<byte>(file_size_bytes),
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
    // for (size, entry) in MftEntryIterator::new(mft_bytes) {
    //     let messages = process_mft_entry(index, size, entry);
    //     for message in messages {
    //         tx.send(message)
    //             .map_err(|e| eyre::eyre!("Failed to send message: {}", e))?;
    //     }
    // }
    let mut parser = MftParser::from_buffer(mft_bytes)
        .map_err(|e| eyre::eyre!("Failed to parse MFT bytes: {}", e))?;
    let size = parser.entry_size as u64;
    for entry in parser.iter_entries() {
        let size = Information::new::<byte>(size);
        let messages = process_mft_entry(index, size, entry);
        for message in messages {
            tx.send(message)
                .map_err(|e| eyre::eyre!("Failed to send message: {}", e))?;
        }
    }
    Ok(())
}

pub fn process_mft_entry(
    index: usize,
    entry_size: Information,
    entry: mft::err::Result<MftEntry>,
) -> Vec<MainboundMessage> {
    let mut rtn = Vec::new();
    let entry = match entry {
        Ok(entry) => {
            // Entry parsed successfully
            rtn.push(MainboundMessage::EntryStatus {
                file_index: index,
                is_healthy: true,
            });
            entry
        }
        Err(e) => {
            // Entry parsing failed
            rtn.push(MainboundMessage::EntryStatus {
                file_index: index,
                is_healthy: false,
            });
            rtn.push(MainboundMessage::Progress {
                file_index: index,
                processed_size: entry_size,
            });
            rtn.push(MainboundMessage::Error {
                file_index: index,
                error: Line::from(format!("Error processing entry: {}", e)),
            });
            return rtn;
        }
    };
    if let Some(path) = get_path_from_entry(&entry) {
        rtn.push(MainboundMessage::DiscoveredFiles {
            file_index: index,
            files: vec![path],
        });
    }
    rtn.push(MainboundMessage::Progress {
        file_index: index,
        processed_size: entry_size,
    });
    rtn
}

pub fn get_path_from_entry(entry: &MftEntry) -> Option<PathBuf> {
    // Analyze attributes
    for attribute in entry.iter_attributes() {
        let Ok(attribute) = attribute else {
            continue;
        };

        if let MftAttributeContent::AttrX30(filename_attribute) = &attribute.data {
            let filename = &filename_attribute.name;
            if !filename.is_empty()
                && !filename.starts_with('$')
                && !filename.eq(".")
                && filename.len() > 2
                && (filename.contains('.') || filename.len() > 8)
            {
                return Some(filename.into());
            }
        }
    }
    None
}
