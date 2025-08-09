use super::drive_letter_pattern::DriveLetterPattern;
use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use eyre;
use std::ffi::OsString;
use std::path::PathBuf;
// Added for parallel drive dumping
use rayon::prelude::*;

/// Arguments for dumping MFT from an NTFS drive
#[derive(Args, Clone, PartialEq, Debug)]
pub struct MftDumpArgs {
    /// Drive letter(s) to dump MFT from. Use '*' for all available drives, or specify one or more drive letters (e.g., 'C', 'D E', 'C,D,E')
    #[clap(default_value_t = DriveLetterPattern::default())]
    pub drive_letters: DriveLetterPattern,

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

        // Generate drive letters pattern
        let drive_letters = DriveLetterPattern::arbitrary(u)?;

        Ok(MftDumpArgs {
            drive_letters,
            output_path,
            overwrite_existing,
        })
    }
}

impl MftDumpArgs {
    pub fn run(self) -> eyre::Result<()> {
        let drives = self.drive_letters.resolve()?;

        if drives.len() > 1 {
            let output_str = self.output_path.to_string_lossy().into_owned();
            if !output_str.contains("%s") {
                return Err(eyre::eyre!(
                    "Output path must contain '%s' placeholder when multiple drives are specified. Found drives: {}",
                    drives.iter().collect::<String>()
                ));
            }
            let overwrite_existing = self.overwrite_existing;
            // Parallel processing of drives
            drives.par_iter().try_for_each(|drive| {
                let drive_output_path = output_str.replace("%s", &drive.to_string());
                crate::mft_dump::dump_mft_to_file(&drive_output_path, overwrite_existing, *drive)
            })?;
        } else if drives.len() == 1 {
            crate::mft_dump::dump_mft_to_file(&self.output_path, self.overwrite_existing, drives[0])?;
        } else {
            return Err(eyre::eyre!("No valid drives found for: {}", self.drive_letters));
        }
        Ok(())
    }
}

impl ToArgs for MftDumpArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.drive_letters.to_string().into());
        args.push(self.output_path.as_os_str().into());
        if self.overwrite_existing { args.push("--overwrite-existing".into()); }
        args
    }
}
