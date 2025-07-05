use mft::MftParser;
use mft::attribute::MftAttributeContent;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn summarize_mft_file(
    mft_file: PathBuf,
    verbose: bool,
    show_paths: bool,
    max_entries: Option<usize>,
) -> eyre::Result<()> {
    println!("Summarizing MFT file: {}", mft_file.display());
    println!();

    // Open and parse the MFT file
    let mut parser = MftParser::from_path(&mft_file)?;
    
    let mut total_entries = 0;
    let mut valid_entries = 0;
    let mut error_entries = 0;
    let mut attribute_counts: HashMap<String, usize> = HashMap::new();
    let mut filename_entries = 0;
    let directory_entries = 0;
    let mut file_entries = 0;
    let mut sample_paths: Vec<String> = Vec::new();

    println!("Analyzing MFT entries...");
    
    for (entry_index, entry_result) in parser.iter_entries().enumerate() {
        // Check if we've hit the maximum entry limit
        if let Some(max) = max_entries {
            if entry_index >= max {
                println!("Reached maximum entry limit of {}, stopping analysis.", max);
                break;
            }
        }

        total_entries += 1;

        // Print progress every 10,000 entries
        if total_entries % 10000 == 0 {
            print!("Processed {} entries...\r", total_entries);
            std::io::Write::flush(&mut std::io::stdout()).unwrap_or(());
        }

        match entry_result {
            Ok(entry) => {
                valid_entries += 1;
                
                // Simple check - we'll just count entries for now
                // TODO: Add proper directory/file classification once we understand the API
                file_entries += 1;

                // Analyze attributes
                for attribute_result in entry.iter_attributes() {
                    match attribute_result {
                        Ok(attribute) => {
                            // Count attribute types
                            let attr_type = format!("{:?}", attribute.header.type_code);
                            *attribute_counts.entry(attr_type).or_insert(0) += 1;

                            // Analyze specific attribute types
                            match &attribute.data {
                                MftAttributeContent::AttrX30(filename_attr) => {
                                    filename_entries += 1;
                                    
                                    // Collect sample file paths if requested
                                    if show_paths && sample_paths.len() < 20 {
                                        let filename = &filename_attr.name;
                                        // Skip system files and collect more interesting paths
                                        if !filename.is_empty() 
                                            && !filename.starts_with('$') 
                                            && !filename.eq(".") 
                                            && filename.len() > 2 
                                            && (filename.contains('.') || filename.len() > 8) {
                                            sample_paths.push(filename.clone());
                                        }
                                    }
                                    
                                    if verbose {
                                        // Could collect filenames here for verbose output
                                    }
                                }
                                _ => {}
                            }
                        }
                        Err(_) => {
                            // Attribute parsing error - still count the entry as valid
                        }
                    }
                }
            }
            Err(_) => {
                error_entries += 1;
            }
        }
    }

    // Clear the progress line
    println!("{}", " ".repeat(50));
    
    // Print summary
    println!("=== MFT Summary ===");
    println!("File: {}", mft_file.display());
    println!();
    
    println!("Entry Statistics:");
    println!("  Total entries processed: {}", total_entries);
    println!("  Valid entries: {}", valid_entries);
    println!("  Error entries: {}", error_entries);
    println!("  Success rate: {:.2}%", (valid_entries as f64 / total_entries as f64) * 100.0);
    println!();
    
    println!("File System Structure:");
    println!("  Directory entries: {} (detection not yet implemented)", directory_entries);
    println!("  File entries: {} (includes all entries for now)", file_entries);
    println!("  Filename attributes: {}", filename_entries);
    println!();
    
    println!("Attribute Statistics:");
    println!("  Total attributes found: {}", attribute_counts.values().sum::<usize>());
    println!();

    if verbose {
        println!("Attribute Type Breakdown:");
        let mut sorted_attributes: Vec<_> = attribute_counts.iter().collect();
        sorted_attributes.sort_by(|a, b| b.1.cmp(a.1));
        
        for (attr_type, count) in sorted_attributes {
            println!("  {:20}: {}", attr_type, count);
        }
        println!();
    }

    if show_paths && !sample_paths.is_empty() {
        println!("Sample File Paths (first {} interesting files found):", sample_paths.len().min(10));
        for (i, path) in sample_paths.iter().take(10).enumerate() {
            println!("  {}: {}", i + 1, path);
        }
        println!();
    }

    if let Some(max) = max_entries {
        if total_entries >= max {
            println!("Note: Analysis was limited to {} entries. The actual MFT may contain more entries.", max);
        }
    }

    Ok(())
}
