use mft::MftParser;
use mft::attribute::MftAttributeContent;
use nucleo::Nucleo;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
struct FileEntry {
    filename: String,
    display_path: String,
    parent_reference: Option<u64>,
    entry_id: u64,
}

#[derive(Clone)]
struct DirectoryEntry {
    name: String,
    parent_reference: Option<u64>,
}

pub fn query_mft_files_fuzzy(
    mft_file: PathBuf,
    query: String,
    limit: usize,
    ignore_case: bool,
    full_paths: bool,
) -> eyre::Result<()> {
    if query.trim().is_empty() {
        return Err(eyre::eyre!(
            "No search query specified. Please provide a search term for fuzzy matching."
        ));
    }

    println!("Fuzzy searching for: '{}'", query);
    println!("Target file: {}", mft_file.display());
    if ignore_case {
        println!("Case-insensitive matching enabled");
    }
    println!();

    // Set up nucleo matcher
    let config = nucleo::Config::DEFAULT;
    let mut matcher = Nucleo::new(
        config,
        Arc::new(|| {}), // notify callback
        None,            // use default number of threads
        1,               // single column for matching
    );

    // Open and parse the MFT file to collect all filenames
    let mut parser = MftParser::from_path(&mft_file)?;
    let mut total_entries = 0;
    let mut files_collected = 0;

    // Directory lookup table for path reconstruction
    let mut directories: HashMap<u64, DirectoryEntry> = HashMap::new();

    println!("Collecting files from MFT...");

    let injector = matcher.injector();

    // First pass: collect all filenames
    for entry_result in parser.iter_entries() {
        total_entries += 1;

        // Print progress every 50,000 entries
        if total_entries % 50000 == 0 {
            print!("Processed {total_entries} entries, collected {files_collected} files...\r");
            std::io::Write::flush(&mut std::io::stdout()).unwrap_or(());
        }

        if let Ok(entry) = entry_result {
            for attribute_result in entry.iter_attributes() {
                if let Ok(attribute) = attribute_result
                    && let MftAttributeContent::AttrX30(filename_attr) = &attribute.data
                {
                    let filename = &filename_attr.name;

                    // Skip system files and very short names
                    if filename.starts_with('$')
                        || filename.len() <= 2
                        || filename == "."
                        || filename == ".."
                    {
                        continue;
                    }

                    let parent_ref = if filename_attr.parent.entry == 0 {
                        None
                    } else {
                        Some(filename_attr.parent.entry)
                    };

                    // For now, treat everything as a potential file and collect directory info separately
                    // Store potential directory information for path reconstruction
                    directories.insert(
                        entry.header.record_number,
                        DirectoryEntry {
                            name: filename.clone(),
                            parent_reference: parent_ref,
                        },
                    );

                    // Add all files to the search index (not distinguishing directories for now)
                    let display_path = if full_paths {
                        reconstruct_full_path(filename, parent_ref, &directories)
                    } else {
                        filename.clone()
                    };

                    let entry_record = FileEntry {
                        filename: filename.clone(),
                        display_path,
                        parent_reference: parent_ref,
                        entry_id: entry.header.record_number,
                    };

                    injector.push(entry_record, |entry_record, columns| {
                        // Use filename for matching, potentially with case adjustment
                        let search_text = if ignore_case {
                            entry_record.filename.to_lowercase()
                        } else {
                            entry_record.filename.clone()
                        };
                        columns[0] = search_text.into();
                    });

                    files_collected += 1;
                }
            }
        }
    }

    // Clear the progress line
    print!("{}\r", " ".repeat(70));

    println!("Collected {files_collected} files from {total_entries} MFT entries");
    println!("Performing fuzzy search...");

    // Set up the search pattern
    let search_query = if ignore_case {
        query.to_lowercase()
    } else {
        query.clone()
    };

    matcher.pattern.reparse(
        0, // column 0
        &search_query,
        if ignore_case {
            nucleo::pattern::CaseMatching::Ignore
        } else {
            nucleo::pattern::CaseMatching::Respect
        },
        nucleo::pattern::Normalization::Smart,
        false, // assume new pattern
    );

    // Wait for matching to complete
    let mut last_matched = 0;
    loop {
        matcher.tick(10); // 10ms timeout
        let snapshot = matcher.snapshot();
        let matched_count = snapshot.matched_item_count() as usize;

        if matched_count != last_matched {
            print!("Matching... found {} results\r", matched_count);
            std::io::Write::flush(&mut std::io::stdout()).unwrap_or(());
            last_matched = matched_count;
        }

        // Check if matching is complete (no change for a few iterations)
        if matched_count > 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let new_snapshot = matcher.snapshot();
            if new_snapshot.matched_item_count() == matched_count as u32 {
                break;
            }
        } else if files_collected == 0 {
            break;
        }

        // Timeout after reasonable time
        if matcher.snapshot().item_count() > 0 {
            break;
        }
    }

    // Clear the progress line
    print!("{}\r", " ".repeat(50));

    // Get and display results
    let snapshot = matcher.snapshot();
    let matched_count = snapshot.matched_item_count() as usize;

    if matched_count == 0 {
        println!("No files found matching the search query '{}'", query);
        println!("Searched {} files total.", files_collected);
        return Ok(());
    }

    println!("Found {} matching files:", matched_count);
    println!();

    let results_to_show = matched_count.min(limit);
    let matched_items = snapshot.matched_items(0..results_to_show as u32);

    for (i, item) in matched_items.enumerate() {
        println!("{}", item.data.display_path);

        if i + 1 >= limit {
            break;
        }
    }

    if matched_count > limit {
        println!();
        println!("... and {} more results (showing first {} due to limit)", 
                matched_count - limit, limit);
    }

    println!();
    println!("Found {} files matching '{}' (limit: {})", 
            matched_count, query, limit);

    Ok(())
}

fn reconstruct_full_path(
    filename: &str,
    parent_ref: Option<u64>,
    directories: &HashMap<u64, DirectoryEntry>,
) -> String {
    let mut path_components = vec![filename.to_string()];
    let mut current_parent = parent_ref;

    // Walk up the directory tree
    while let Some(parent_id) = current_parent {
        if let Some(parent_dir) = directories.get(&parent_id) {
            // Skip root directory references
            if parent_dir.name == "." || parent_id == 5 {
                break;
            }
            path_components.push(parent_dir.name.clone());
            current_parent = parent_dir.parent_reference;
        } else {
            break;
        }
    }

    // Reverse to get correct order (root to file) and join with backslashes
    path_components.reverse();
    format!("\\{}", path_components.join("\\"))
}
