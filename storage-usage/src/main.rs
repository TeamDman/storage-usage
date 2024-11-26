use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Storage::FileSystem::GetLogicalDriveStringsW;

fn main() {
    // Buffer to hold drive strings (UTF-16 encoded)
    let mut buffer = vec![0u16; 256];

    unsafe {
        // Get the logical drive strings
        let result = GetLogicalDriveStringsW(Some(&mut buffer));

        if result == 0 {
            eprintln!("Failed to get logical drives. Error: {:?}", GetLastError());
            return;
        }

        // Parse the buffer into individual drive strings
        let drives = parse_drive_strings(&buffer[..result as usize]);

        for drive in drives {
            println!("Drive: {}", drive.to_string_lossy());

            // Read the entries in the root of the drive
            match fs::read_dir(&drive) {
                Ok(entries) => {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let path = entry.path();

                            // Get metadata to determine if it's a file or directory and get size
                            match entry.metadata() {
                                Ok(metadata) => {
                                    if metadata.is_file() {
                                        let size = metadata.len();
                                        println!(" - {} (File, {} bytes)", path.display(), size);
                                    } else if metadata.is_dir() {
                                        // For directories, you can choose to calculate size recursively
                                        println!(" - {} (Directory)", path.display());
                                    } else {
                                        println!(" - {} (Other)", path.display());
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to get metadata for {}: {}", path.display(), e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Failed to read directory {}: {}",
                        drive.to_string_lossy(),
                        e
                    );
                }
            }
            println!(); // Add an empty line for readability
        }
    }
}

// Function to parse the buffer returned by GetLogicalDriveStringsW into a Vec of OsString
fn parse_drive_strings(buffer: &[u16]) -> Vec<OsString> {
    let mut drives = Vec::new();
    let mut start = 0;

    for (i, &c) in buffer.iter().enumerate() {
        if c == 0 {
            if start != i {
                let os_string = OsString::from_wide(&buffer[start..i]);
                drives.push(os_string);
            }
            start = i + 1;
        }
    }

    drives
}
