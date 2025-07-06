use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use std::ffi::OsString;
use std::path::PathBuf;

/// Arguments for dumping MFT from an NTFS drive
#[derive(Args, Clone, PartialEq, Debug)]
pub struct MftDumpArgs {
    /// Path where the MFT dump file will be saved
    pub output_path: PathBuf,

    #[clap(long, help = "Overwrite existing output file")]
    pub overwrite_existing: bool,

    #[clap(
        long,
        short = 'd',
        default_value = "C",
        help = "Drive letter to dump MFT from"
    )]
    pub drive_letter: char,
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

        // Generate a valid drive letter (A-Z)
        let drive_letter = {
            let letter_index = u8::arbitrary(u)? % 26;
            (b'A' + letter_index) as char
        };

        Ok(MftDumpArgs {
            output_path,
            overwrite_existing,
            drive_letter,
        })
    }
}

impl MftDumpArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::mft_dump::dump_mft_to_file(
            self.output_path,
            self.overwrite_existing,
            self.drive_letter,
        )
    }
}

impl ToArgs for MftDumpArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.output_path.as_os_str().into());
        if self.overwrite_existing {
            args.push("--overwrite-existing".into());
        }
        args.push("--drive-letter".into());
        args.push(self.drive_letter.to_string().into());
        args
    }
}
