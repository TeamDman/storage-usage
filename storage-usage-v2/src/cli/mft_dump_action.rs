use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use eyre;
use std::ffi::OsString;
use std::path::PathBuf;

/// Parse drive letters from input string, handling wildcards and multiple drives
fn parse_drive_letters(input: &str) -> eyre::Result<Vec<char>> {
    let input = input.trim();

    if input == "*" {
        // Get all available drives
        return get_available_drives();
    }

    // Parse individual drive letters
    let mut drives = Vec::new();

    // Handle various separators: space, comma, semicolon
    let parts: Vec<&str> = input
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|s| !s.is_empty())
        .collect();

    for part in parts {
        let part = part.trim();
        if part.len() == 1 {
            if let Some(drive_char) = part.chars().next() {
                if drive_char.is_ascii_alphabetic() {
                    drives.push(drive_char.to_ascii_uppercase());
                } else {
                    return Err(eyre::eyre!("Invalid drive letter: '{}'", part));
                }
            }
        } else if part.len() > 1 {
            // Handle multiple characters as individual drive letters
            for drive_char in part.chars() {
                if drive_char.is_ascii_alphabetic() {
                    drives.push(drive_char.to_ascii_uppercase());
                } else {
                    return Err(eyre::eyre!("Invalid drive letter: '{}'", drive_char));
                }
            }
        }
    }

    if drives.is_empty() {
        return Err(eyre::eyre!("No valid drive letters found in: '{}'", input));
    }

    Ok(drives)
}

/// Get all available drives on the system
fn get_available_drives() -> eyre::Result<Vec<char>> {
    use windows::Win32::Storage::FileSystem::GetLogicalDrives;

    let drives_bitmask = unsafe { GetLogicalDrives() };

    let mut available_drives = Vec::new();
    for i in 0..26 {
        if (drives_bitmask & (1 << i)) != 0 {
            available_drives.push((b'A' + i as u8) as char);
        }
    }

    if available_drives.is_empty() {
        return Err(eyre::eyre!("No drives found on system"));
    }

    Ok(available_drives)
}

/// Arguments for dumping MFT from an NTFS drive
#[derive(Args, Clone, PartialEq, Debug)]
pub struct MftDumpArgs {
    /// Drive letter(s) to dump MFT from. Use '*' for all available drives, or specify one or more drive letters (e.g., 'C', 'D E', 'C,D,E')
    pub drive_letters: String,

    /// Path where the MFT dump file will be saved. Use %s for drive letter substitution when multiple drives are specified
    pub output_path: PathBuf,

    #[clap(long, help = "Overwrite existing output file")]
    pub overwrite_existing: bool,
}

impl<'a> Arbitrary<'a> for MftDumpArgs {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        // Generate a valid non-empty path
        let output_path = {
            let path_chars: Vec<char> = (0..10)
                .map(|_| {
                    let c = char::arbitrary(u).unwrap_or('a');
                    if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
                        c
                    } else {
                        'a'
                    }
                })
                .collect();
            let path_str: String = path_chars.into_iter().collect();
            format!("test_{path_str}.txt").into()
        };

        // Generate a random boolean for overwrite_existing
        let overwrite_existing = bool::arbitrary(u)?;

        // Generate drive letters string (A-Z)
        let drive_letters = {
            let letter_index = u8::arbitrary(u)? % 26;
            let letter = (b'A' + letter_index) as char;
            letter.to_string()
        };

        Ok(MftDumpArgs {
            drive_letters,
            output_path,
            overwrite_existing,
        })
    }
}

impl MftDumpArgs {
    pub fn run(self) -> eyre::Result<()> {
        let drives = parse_drive_letters(&self.drive_letters)?;

        if drives.len() > 1 {
            // Multiple drives - validate output path contains %s
            let output_str = self.output_path.to_string_lossy();
            if !output_str.contains("%s") {
                return Err(eyre::eyre!(
                    "Output path must contain '%s' placeholder when multiple drives are specified. Found drives: {}",
                    drives.iter().collect::<String>()
                ));
            }

            // Process each drive
            for drive in drives {
                let drive_output_path = output_str.replace("%s", &drive.to_string());
                crate::mft_dump::dump_mft_to_file(
                    &drive_output_path,
                    self.overwrite_existing,
                    drive,
                )?;
            }
        } else if drives.len() == 1 {
            // Single drive
            crate::mft_dump::dump_mft_to_file(
                &self.output_path,
                self.overwrite_existing,
                drives[0],
            )?;
        } else {
            return Err(eyre::eyre!(
                "No valid drives found for: {}",
                self.drive_letters
            ));
        }

        Ok(())
    }
}

impl ToArgs for MftDumpArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.drive_letters.clone().into());
        args.push(self.output_path.as_os_str().into());
        if self.overwrite_existing {
            args.push("--overwrite-existing".into());
        }
        args
    }
}
