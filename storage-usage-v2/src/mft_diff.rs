use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;

pub fn diff_mft_files(
    file1: PathBuf,
    file2: PathBuf,
    verbose: bool,
    max_diffs: Option<usize>,
) -> eyre::Result<()> {
    println!("Comparing MFT files:");
    println!("  File 1: {}", file1.display());
    println!("  File 2: {}", file2.display());
    println!();

    // Open both files
    let file1_handle = File::open(&file1)?;
    let file2_handle = File::open(&file2)?;

    let mut reader1 = BufReader::new(file1_handle);
    let mut reader2 = BufReader::new(file2_handle);

    // Get file sizes
    let metadata1 = std::fs::metadata(&file1)?;
    let metadata2 = std::fs::metadata(&file2)?;

    let size1 = metadata1.len();
    let size2 = metadata2.len();

    println!("File sizes:");
    println!("  File 1: {size1} bytes");
    println!("  File 2: {size2} bytes");
    println!(
        "  Difference: {} bytes",
        (size1 as i64 - size2 as i64).abs()
    );
    println!();

    // Read files in chunks and compare
    let mut buffer1 = [0u8; 4096];
    let mut buffer2 = [0u8; 4096];
    let mut position = 0u64;
    let mut differences_found = 0usize;
    let max_diffs_to_show = max_diffs.unwrap_or(10);
    let mut first_difference: Option<u64> = None;

    loop {
        let bytes_read1 = reader1.read(&mut buffer1)?;
        let bytes_read2 = reader2.read(&mut buffer2)?;

        // If one file is shorter than the other
        if bytes_read1 != bytes_read2 {
            let shorter_file = if bytes_read1 < bytes_read2 { 1 } else { 2 };
            let longer_file = if bytes_read1 < bytes_read2 { 2 } else { 1 };

            println!("Files differ in length:");
            println!(
                "  File {} ends at position {}",
                shorter_file,
                position + bytes_read1.min(bytes_read2) as u64
            );
            println!("  File {longer_file} continues beyond this point");
            break;
        }

        // If we've reached the end of both files
        if bytes_read1 == 0 {
            break;
        }

        // Compare the chunks byte by byte
        for i in 0..bytes_read1 {
            if buffer1[i] != buffer2[i] {
                let byte_position = position + i as u64;

                if first_difference.is_none() {
                    first_difference = Some(byte_position);
                }

                if verbose && differences_found < max_diffs_to_show {
                    println!(
                        "Difference at byte {}: 0x{:02X} vs 0x{:02X} (decimal: {} vs {})",
                        byte_position, buffer1[i], buffer2[i], buffer1[i], buffer2[i]
                    );
                }

                differences_found += 1;

                if differences_found >= max_diffs_to_show && verbose {
                    println!(
                        "... and {} more differences (use --max-diffs to see more)",
                        count_remaining_differences(
                            &mut reader1,
                            &mut reader2,
                            position + bytes_read1 as u64
                        )?
                    );
                    break;
                }
            }
        }

        position += bytes_read1 as u64;

        // Early exit if we've found enough differences and not in verbose mode
        if !verbose && differences_found > 0 {
            break;
        }
    }

    println!("Summary:");
    if differences_found == 0 {
        println!("  Files are identical!");
    } else {
        if let Some(first_diff_pos) = first_difference {
            println!("  First difference at byte: {first_diff_pos}");
            println!(
                "  As percentage of file: {:.2}%",
                (first_diff_pos as f64 / size1.min(size2) as f64) * 100.0
            );
        }
        println!("  Total differences found: {differences_found}");

        if differences_found == 1 {
            println!("  Files are very similar (only 1 byte differs)");
        } else if first_difference.unwrap_or(0) < 1024 {
            println!("  Files diverge very early (likely different headers/metadata)");
        } else {
            println!("  Files are mostly similar initially, then diverge");
        }
    }

    Ok(())
}

fn count_remaining_differences(
    reader1: &mut BufReader<File>,
    reader2: &mut BufReader<File>,
    _start_position: u64,
) -> eyre::Result<usize> {
    let mut buffer1 = [0u8; 4096];
    let mut buffer2 = [0u8; 4096];
    let mut remaining_diffs = 0;

    loop {
        let bytes_read1 = reader1.read(&mut buffer1)?;
        let bytes_read2 = reader2.read(&mut buffer2)?;

        if bytes_read1 != bytes_read2 {
            // Files differ in length, count as many differences as the extra bytes
            remaining_diffs += (bytes_read1 as i32 - bytes_read2 as i32).unsigned_abs() as usize;
            break;
        }

        if bytes_read1 == 0 {
            break;
        }

        for i in 0..bytes_read1 {
            if buffer1[i] != buffer2[i] {
                remaining_diffs += 1;
            }
        }
    }

    Ok(remaining_diffs)
}
