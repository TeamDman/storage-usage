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

#[cfg(test)]
mod test {
    use crate::mft_entry_iterator::MftEntryIterator;
    use mft::MftParser;
    use std::path::PathBuf;

    #[test]
    fn it_works() -> eyre::Result<()> {
        let mft_path = PathBuf::from(r"C:\Users\TeamD\Downloads\storage\mine-I.mft");
        let data = unsafe {
            memmap2::MmapOptions::new()
                .map(&std::fs::File::open(&mft_path)?)
                .map_err(|e| eyre::eyre!("Failed to memory map MFT file: {}", e))
        }?;
        let ours = MftEntryIterator::new(&data);

        let mut parser = MftParser::from_buffer(data.to_vec())?;
        let theirs = parser.iter_entries();

        for (i, ((_, left), right)) in ours.zip(theirs).enumerate() {
            match (left, right) {
                (Ok(left_entry), Ok(right_entry)) => {
                    if left_entry.header != right_entry.header {
                        println!("Entry {}: Headers differ", i);
                        println!("Our header: {:?}", left_entry.header);
                        println!("Their header: {:?}", right_entry.header);
                        panic!("Headers don't match at entry {}", i);
                    }
                    if left_entry.data != right_entry.data {
                        println!("Entry {}: Data differs", i);
                        println!("Our data length: {}", left_entry.data.len());
                        println!("Their data length: {}", right_entry.data.len());
                        if left_entry.data.len() == right_entry.data.len() {
                            // Find first difference
                            for (j, (a, b)) in left_entry
                                .data
                                .iter()
                                .zip(right_entry.data.iter())
                                .enumerate()
                            {
                                if a != b {
                                    println!("First difference at byte {}: {} vs {}", j, a, b);
                                    break;
                                }
                            }
                        }
                        panic!("Data doesn't match at entry {}", i);
                    }
                }
                (Err(left_err), Err(right_err)) => {
                    let left_str = left_err.to_string();
                    let right_str = right_err.to_string();
                    if left_str != right_str {
                        println!("Entry {}: Error messages differ", i);
                        println!("Our error: {}", left_str);
                        println!("Their error: {}", right_str);
                        panic!("Error messages don't match at entry {}", i);
                    }
                }
                (Ok(_), Err(err)) => {
                    println!("Entry {}: We succeeded, they failed with: {}", i, err);
                    panic!("Entries do not match at entry {}", i);
                }
                (Err(err), Ok(_)) => {
                    println!("Entry {}: We failed with: {}, they succeeded", i, err);
                    panic!("Entries do not match at entry {}", i);
                }
            }
        }

        Ok(())
    }
}
