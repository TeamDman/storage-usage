use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Args, Clone, PartialEq, Debug, Arbitrary)]
pub struct MftSummarizeArgs {
    #[clap(help = "Path to the MFT file to summarize")]
    pub mft_file: PathBuf,

    #[clap(long, help = "Show detailed statistics about MFT entries")]
    pub verbose: bool,

    #[clap(long, help = "Show sample file paths from the MFT")]
    pub show_paths: bool,

    #[clap(long, help = "Maximum number of entries to process (for testing on large files)")]
    pub max_entries: Option<usize>,
}

impl MftSummarizeArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::mft_summarize::summarize_mft_file(self.mft_file, self.verbose, self.show_paths, self.max_entries)
    }
}

impl ToArgs for MftSummarizeArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.mft_file.as_os_str().into());
        
        if self.verbose {
            args.push("--verbose".into());
        }

        if self.show_paths {
            args.push("--show-paths".into());
        }
        
        if let Some(max_entries) = self.max_entries {
            args.push("--max-entries".into());
            args.push(max_entries.to_string().into());
        }
        
        args
    }
}
