use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use std::ffi::OsString;
use std::path::PathBuf;

/// Arguments for comparing two MFT files
#[derive(Args, Clone, PartialEq, Debug, Arbitrary)]
pub struct MftDiffArgs {
    #[clap(help = "First MFT file to compare")]
    pub file1: PathBuf,

    #[clap(help = "Second MFT file to compare")]
    pub file2: PathBuf,

    #[clap(long, help = "Show detailed byte-by-byte differences")]
    pub verbose: bool,

    #[clap(long, help = "Maximum number of differences to show (default: 10)")]
    pub max_diffs: Option<usize>,
}

impl MftDiffArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::mft_diff::diff_mft_files(self.file1, self.file2, self.verbose, self.max_diffs)
    }
}

impl ToArgs for MftDiffArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.file1.as_os_str().into());
        args.push(self.file2.as_os_str().into());

        if self.verbose {
            args.push("--verbose".into());
        }

        if let Some(max_diffs) = self.max_diffs {
            args.push("--max-diffs".into());
            args.push(max_diffs.to_string().into());
        }

        args
    }
}
