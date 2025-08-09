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
    parent_ref: Option<u64>,
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

pub fn query_mft_files_fuzzy(drive_pattern: DriveLetterPattern, query: String, limit: usize, display_interval: Duration, top_n: usize, timeout: Option<Duration>) -> eyre::Result<()> {
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
    let drives_cloned = drives.clone();
    std::thread::spawn(move || {
        // Structure holding a not-yet-resolved entry
        #[derive(Clone)]
        struct PendingEntry {
            record_number: u64,
            filename: String,
            parent_ref: Option<u64>,
            created: Option<DateTime<Utc>>,
            modified: Option<DateTime<Utc>>,
            accessed: Option<DateTime<Utc>>,
        }

        mft_files_cloned.par_iter().enumerate().for_each(|(drive_index, mft_file)| {
            if let Ok(mut parser) = MftParser::from_path(mft_file) {
                let drive_letter = drives_cloned[drive_index];
                let mut directories: HashMap<u64, DirectoryEntry> = HashMap::new();
                // parent_id -> list of children waiting for that ancestor to appear
                let mut pending: HashMap<u64, Vec<PendingEntry>> = HashMap::new();

                // Attempt to resolve a vector of pending entries (called when a new directory becomes available)
                let mut resolve_queue = Vec::new();

                for entry_result in parser.iter_entries() {
                    worker_total.fetch_add(1, Ordering::Relaxed);
                    if let Ok(entry) = entry_result {
                        let record_number = entry.header.record_number;
                        let mut std_created = None;
                        let mut std_modified = None;
                        let mut std_accessed = None;
                        for attribute_result in entry.iter_attributes() {
                            if let Ok(attribute) = attribute_result
                                && let MftAttributeContent::AttrX10(info) = &attribute.data
                            {
                                std_created = Some(info.created);
                                std_modified = Some(info.modified);
                                std_accessed = Some(info.accessed);
                                break;
                            }
                        }
                        for attribute_result in entry.iter_attributes() {
                            if let Ok(attribute) = attribute_result
                                && let MftAttributeContent::AttrX30(filename_attr) = &attribute.data
                            {
                                let filename = &filename_attr.name;
                                if filename.starts_with('$') || filename == "." || filename == ".." { continue; }
                                let parent_ref = if filename_attr.parent.entry == 0 { None } else { Some(filename_attr.parent.entry) };

                                // Insert directory entry for this record (even if it's a file; harmless, enables parent traversal)
                                directories.insert(record_number, DirectoryEntry { name: filename.clone(), parent_reference: parent_ref });

                                // Try to build full path now
                                match try_build_full_path(filename, parent_ref, &directories, drive_letter) {
                                    Ok(full_path) => {
                                        let entry_record = FileEntry {
                                            filename: filename.clone(),
                                            parent_ref,
                                            display_path: full_path,
                                            created: Some(filename_attr.created).or(std_created),
                                            modified: Some(filename_attr.modified).or(std_modified),
                                            accessed: Some(filename_attr.accessed).or(std_accessed),
                                        };
                                        injector.push(entry_record, |e, cols| { cols[0] = e.display_path.clone().into(); });
                                        worker_files.fetch_add(1, Ordering::Relaxed);

                                        // Newly inserted directory might unblock children waiting on this record_number
                                        if let Some(children) = pending.remove(&record_number) {
                                            resolve_queue.extend(children);
                                        }
                                    }
                                    Err(missing_parent) => {
                                        // Queue for later when that parent id appears
                                        let p = PendingEntry {
                                            record_number,
                                            filename: filename.clone(),
                                            parent_ref,
                                            created: Some(filename_attr.created).or(std_created),
                                            modified: Some(filename_attr.modified).or(std_modified),
                                            accessed: Some(filename_attr.accessed).or(std_accessed),
                                        };
                                        pending.entry(missing_parent).or_default().push(p);
                                    }
                                }

                                // Resolve queue breadth-first
                                while let Some(pend) = resolve_queue.pop() {
                                    match try_build_full_path(&pend.filename, pend.parent_ref, &directories, drive_letter) {
                                        Ok(path) => {
                                            let entry_record = FileEntry {
                                                filename: pend.filename.clone(),
                                                parent_ref: pend.parent_ref,
                                                display_path: path,
                                                created: pend.created,
                                                modified: pend.modified,
                                                accessed: pend.accessed,
                                            };
                                            injector.push(entry_record, |e, cols| { cols[0] = e.display_path.clone().into(); });
                                            worker_files.fetch_add(1, Ordering::Relaxed);
                                            if let Some(children) = pending.remove(&pend.record_number) {
                                                resolve_queue.extend(children);
                                            }
                                        }
                                        Err(missing_parent) => {
                                            pending.entry(missing_parent).or_default().push(pend);
                                        }
                                    }
                                }
                                break; // first X30 only
                            }
                        }
                    }
                }

                // Any remaining pending entries couldn't resolve (cycles or missing ancestors); inject best-effort partials
                for (_missing, entries) in pending.into_iter() {
                    for pend in entries {
                        let partial_path = format!("{drive_letter}:\\{}", pend.filename); // minimal fallback
                        let entry_record = FileEntry {
                            filename: pend.filename,
                            parent_ref: pend.parent_ref,
                            display_path: partial_path,
                            created: pend.created,
                            modified: pend.modified,
                            accessed: pend.accessed,
                        };
                        injector.push(entry_record, |e, cols| { cols[0] = e.display_path.clone().into(); });
                        worker_files.fetch_add(1, Ordering::Relaxed);
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
        if let Some(t) = timeout { if start.elapsed() >= t { break; } }
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
                if let Some(t) = timeout { if start.elapsed() >= t { break; } }
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
    if let Some(t) = timeout { if start.elapsed() >= t { println!("Timeout reached after {} ms", start.elapsed().as_millis()); } }
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
        if pid == 5 { // root sentinel
            break;
        }
        if let Some(dir) = directories.get(&pid) {
            if dir.name == "." { break; }
            components.push(dir.name.clone());
            current = dir.parent_reference;
        } else {
            return Err(pid); // missing ancestor
        }
        guard += 1;
    }
    components.reverse();
    Ok(format!("{drive_letter}:\\{}", components.join("\\")))
}
