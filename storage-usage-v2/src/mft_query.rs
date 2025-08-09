// Import chrono types from mft crate's exports
use chrono::{DateTime, Utc};
use mft::MftParser;
use mft::attribute::MftAttributeContent;
use nucleo::Nucleo;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use crate::cli::drive_letter_pattern::DriveLetterPattern; // new
use crate::config::get_cache_dir; // new
use rayon::prelude::*; // new
use std::time::{Duration, Instant}; // added
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering}; // new

#[derive(Clone)]
struct FileEntry {
    filename: String,
    display_path: String,
    created: Option<DateTime<Utc>>,
    modified: Option<DateTime<Utc>>,
    accessed: Option<DateTime<Utc>>,
}

#[derive(Clone)]
struct DirectoryEntry {
    name: String,
    parent_reference: Option<u64>,
}

pub fn query_mft_files_fuzzy(drive_pattern: DriveLetterPattern, query: String, limit: usize, display_interval: Duration, top_n: usize) -> eyre::Result<()> {
    if query.trim().is_empty() {
        return Err(eyre::eyre!(
            "No search query specified. Please provide a search term for fuzzy matching."
        ));
    }

    let drives = drive_pattern.resolve()?;
    let cache = get_cache_dir()?;
    let mut mft_files: Vec<PathBuf> = drives.iter().map(|d| cache.join(format!("{d}.mft"))).collect();
    mft_files.retain(|p| p.exists());

    if mft_files.is_empty() {
        return Err(eyre::eyre!("No cached MFT files found for pattern '{}'. Run mft sync first.", drive_pattern));
    }

    println!("Fuzzy searching for: '{query}'");
    println!("Drives: {}", drives.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","));
    println!("Using full paths for all results");
    println!();

    // Set up nucleo matcher
    let config = nucleo::Config::DEFAULT;
    let mut matcher = Nucleo::new(
        config,
        Arc::new(|| {}), // notify callback
        None,            // default threads
        1,               // single column for matching
    );

    println!("Collecting files from cached MFTs in parallel...");

    // Shared progress counters
    let total_entries = Arc::new(AtomicU64::new(0));
    let files_collected = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let injector = matcher.injector();

    // Spawn worker thread performing parallel parsing & streaming injection
    let worker_total = total_entries.clone();
    let worker_files = files_collected.clone();
    let worker_done = done.clone();
    let mft_files_cloned = mft_files.clone();
    std::thread::spawn(move || {
        mft_files_cloned.par_iter().for_each(|mft_file| {
            if let Ok(mut parser) = MftParser::from_path(mft_file) {
                let mut directories: HashMap<u64, DirectoryEntry> = HashMap::new();
                for entry_result in parser.iter_entries() {
                    worker_total.fetch_add(1, Ordering::Relaxed);
                    if let Ok(entry) = entry_result {
                        let mut std_created = None;
                        let mut std_modified = None;
                        let mut std_accessed = None;
                        for attribute_result in entry.iter_attributes() {
                            if let Ok(attribute) = attribute_result
                                && let MftAttributeContent::AttrX10(standard_info_attr) = &attribute.data
                            {
                                std_created = Some(standard_info_attr.created);
                                std_modified = Some(standard_info_attr.modified);
                                std_accessed = Some(standard_info_attr.accessed);
                                break;
                            }
                        }
                        for attribute_result in entry.iter_attributes() {
                            if let Ok(attribute) = attribute_result
                                && let MftAttributeContent::AttrX30(filename_attr) = &attribute.data
                            {
                                let filename = &filename_attr.name;
                                if filename.starts_with('$') || filename.len() <= 2 || filename == "." || filename == ".." { continue; }
                                let parent_ref = if filename_attr.parent.entry == 0 { None } else { Some(filename_attr.parent.entry) };
                                directories.insert(entry.header.record_number, DirectoryEntry { name: filename.clone(), parent_reference: parent_ref });
                                let display_path = reconstruct_full_path(filename, parent_ref, &directories);
                                let entry_record = FileEntry {
                                    filename: filename.clone(),
                                    display_path,
                                    created: Some(filename_attr.created).or(std_created),
                                    modified: Some(filename_attr.modified).or(std_modified),
                                    accessed: Some(filename_attr.accessed).or(std_accessed),
                                };
                                injector.push(entry_record, |entry_record, columns| {
                                    columns[0] = entry_record.filename.clone().into();
                                });
                                worker_files.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        });
        worker_done.store(true, Ordering::Release);
    });

    println!("Performing fuzzy search & streaming results...");
    matcher.pattern.reparse(
        0,
        &query,
        nucleo::pattern::CaseMatching::Smart,
        nucleo::pattern::Normalization::Smart,
        false,
    );

    let start = Instant::now();
    let mut last_display = Instant::now() - display_interval; // force immediate first display

    // Periodic display until parsing complete
    loop {
        matcher.tick(10); // small wait for matcher updates
        if last_display.elapsed() >= display_interval {
            let snapshot = matcher.snapshot();
            let matched_count = snapshot.matched_item_count() as usize;
            let total = total_entries.load(Ordering::Relaxed);
            let collected = files_collected.load(Ordering::Relaxed);
            let show_n = matched_count.min(top_n);
            println!("--- {} ({} ms elapsed, entries processed: {}, files collected: {}, matches: {}) ---",
                if done.load(Ordering::Acquire) { "Final preview" } else { "Preview" },
                start.elapsed().as_millis(),
                total,
                collected,
                matched_count,
            );
            if matched_count == 0 {
                println!("(no matches yet)");
            } else {
                let preview_items = snapshot.matched_items(0..show_n as u32);
                for item in preview_items { println!("{}", item.data.display_path); }
                if matched_count > show_n { println!("... ({} more preview matches)", matched_count - show_n); }
            }
            println!();
            last_display = Instant::now();
            if done.load(Ordering::Acquire) { break; }
        }
        if done.load(Ordering::Acquire) {
            // ensure a final display if interval not yet elapsed
            if last_display.elapsed() < display_interval {
                continue; // loop will hit display soon
            }
        }
    }

    // Final snapshot & full display up to limit
    matcher.tick(0);
    let snapshot = matcher.snapshot();
    let matched_count = snapshot.matched_item_count() as usize;
    let total_entries_val = total_entries.load(Ordering::Relaxed);
    let files_collected_val = files_collected.load(Ordering::Relaxed);

    if matched_count == 0 {
        println!("No files found matching the search query '{query}'");
        println!("Searched {files_collected_val} files ({} entries) total.", total_entries_val);
        return Ok(());
    }

    println!("Found {matched_count} matching files (processed {files_collected_val} files / {total_entries_val} entries across {} drives):\n", mft_files.len());

    let results_to_show = matched_count.min(limit);
    let matched_items = snapshot.matched_items(0..results_to_show as u32);
    for (i, item) in matched_items.enumerate() {
        let entry = &item.data;
        let created_str = entry.created.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_else(|| "N/A".to_string());
        let modified_str = entry.modified.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_else(|| "N/A".to_string());
        let accessed_str = entry.accessed.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_else(|| "N/A".to_string());
        println!("{}", entry.display_path);
        println!("  Created:  {created_str} UTC");
        println!("  Modified: {modified_str} UTC");
        println!("  Accessed: {accessed_str} UTC\n");
        if i + 1 >= limit { break; }
    }
    if matched_count > limit { println!("\n... and {} more results (showing first {} due to limit)", matched_count - limit, limit); }
    println!("\nFound {matched_count} files matching '{query}' (limit: {limit})");
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
