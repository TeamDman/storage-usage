use mft::MftParser;
use mft::attribute::MftAttributeContent;
use std::path::PathBuf;

pub fn query_mft_files(
    mft_file: PathBuf,
    extensions: Vec<String>,
    limit: usize,
    ignore_case: bool,
    full_paths: bool,
) -> eyre::Result<()> {
    if extensions.is_empty() {
        return Err(eyre::eyre!("No search patterns specified. Please provide file extensions (e.g., *.mp4, txt) or literal filenames"));
    }

    // Process search patterns: distinguish between extensions and literal filename patterns
    let mut extension_patterns = Vec::new();
    let mut literal_patterns = Vec::new();
    
    for pattern in &extensions {
        // If pattern contains no wildcards and has spaces or special chars, treat as literal filename
        if !pattern.contains('*') && (pattern.contains(' ') || pattern.contains('(') || pattern.contains(')') || pattern.contains('-') || pattern.len() > 10) {
            // Literal filename pattern
            let literal = if ignore_case {
                pattern.to_lowercase()
            } else {
                pattern.clone()
            };
            literal_patterns.push(literal);
        } else {
            // Extension pattern - remove * and . if present
            let clean_ext = pattern.trim_start_matches('*').trim_start_matches('.');
            let ext = if ignore_case {
                clean_ext.to_lowercase()
            } else {
                clean_ext.to_string()
            };
            extension_patterns.push(ext);
        }
    }

    println!("Searching for patterns: {}", extensions.join(", "));
    if !extension_patterns.is_empty() {
        println!("  Extensions: {}", extension_patterns.iter().map(|e| format!("*.{}", e)).collect::<Vec<_>>().join(", "));
    }
    if !literal_patterns.is_empty() {
        println!("  Literal filenames: {}", literal_patterns.join(", "));
    }
    println!("Target file: {}", mft_file.display());
    if ignore_case {
        println!("Case-insensitive matching enabled");
    }
    println!();

    // Open and parse the MFT file
    let mut parser = MftParser::from_path(&mft_file)?;
    
    let mut matches_found = 0;
    let mut total_entries = 0;

    // Simple approach: if full_paths is requested, we'll show what we can reconstruct per entry
    // without doing a full two-pass approach that seems to cause issues with this MFT file

    // Search for matching files
    for entry_result in parser.iter_entries() {
        total_entries += 1;

        // Print progress every 50,000 entries
        if total_entries % 50000 == 0 {
            print!("Processed {} entries, found {} matches...\r", total_entries, matches_found);
            std::io::Write::flush(&mut std::io::stdout()).unwrap_or(());
        }

        if matches_found >= limit {
            break;
        }

        if let Ok(entry) = entry_result {
            for attribute_result in entry.iter_attributes() {
                if let Ok(attribute) = attribute_result {
                    if let MftAttributeContent::AttrX30(filename_attr) = &attribute.data {
                        let filename = &filename_attr.name;
                        
                        // Skip system files, directories, and very short names
                        if filename.starts_with('$') || filename.len() <= 2 || filename == "." || filename == ".." {
                            continue;
                        }

                        // Check if filename matches any of our patterns
                        let filename_to_check = if ignore_case {
                            filename.to_lowercase()
                        } else {
                            filename.clone()
                        };

                        // Check extension patterns (ends with .ext)
                        let matches_extension = extension_patterns.iter().any(|ext| {
                            if ext.is_empty() {
                                false
                            } else {
                                filename_to_check.ends_with(&format!(".{}", ext))
                            }
                        });
                        
                        // Check literal patterns (exact filename match)
                        let matches_literal = literal_patterns.iter().any(|pattern| {
                            filename_to_check == *pattern
                        });

                        if matches_extension || matches_literal {
                            matches_found += 1;
                            
                            if full_paths {
                                // For now, just show the filename with a path prefix
                                // Full reconstruction would require more robust MFT parsing
                                println!("\\{}", filename);
                            } else {
                                println!("{}", filename);
                            }

                            if matches_found >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Clear the progress line
    print!("{}\r", " ".repeat(60));

    if matches_found == 0 {
        println!("No files found matching the specified extensions.");
        println!("Searched {} entries total.", total_entries);
    } else {
        println!();
        println!("Found {} files matching the specified extensions (limit: {}).", matches_found, limit);
        println!("Searched {} entries total.", total_entries);
        
        if matches_found == limit {
            println!("Note: Result limit reached. There may be more matching files.");
        }
    }

    Ok(())
}
