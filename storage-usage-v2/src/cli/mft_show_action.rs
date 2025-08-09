use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use std::ffi::OsString;
use crate::config::get_cache_dir; // keep

/// Arguments for generating MFT statistics and summary
#[derive(Args, Clone, PartialEq, Debug, Arbitrary)]
pub struct MftShowArgs {
    #[clap(
        help = "Path pattern to MFT file(s) to analyze. Supports glob patterns like '*.mft', 'dump-*.mft', or '/path/to/*.mft'. If omitted uses cached '*.mft' files.",
        value_name = "PATTERN"
    )]
    pub mft_pattern: Option<String>,

    #[clap(long, help = "Show detailed statistics about MFT entries")]
    pub verbose: bool,

    #[clap(long, help = "Show sample file paths from the MFT")]
    pub show_paths: bool,

    #[clap(
        long,
        help = "Maximum number of entries to process (for testing on large files)"
    )]
    pub max_entries: Option<usize>,

    #[clap(
        long,
        short = 'j',
        help = "Number of threads to use for parallel processing (default: auto-detect)"
    )]
    pub threads: Option<usize>,
}

impl MftShowArgs {
    pub fn run(self) -> eyre::Result<()> {
        let resolved_pattern = match &self.mft_pattern {
            Some(p) => p.clone(),
            None => {
                let cache_dir = get_cache_dir()?;
                cache_dir.join("*.mft").to_string_lossy().to_string()
            }
        };
        crate::mft_show::show_mft_files(
            &resolved_pattern,
            self.verbose,
            self.show_paths,
            self.max_entries,
            self.threads,
        )
    }
}

impl ToArgs for MftShowArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        if let Some(p) = &self.mft_pattern { args.push(p.clone().into()); }
        if self.verbose { args.push("--verbose".into()); }
        if self.show_paths { args.push("--show-paths".into()); }
        if let Some(max_entries) = self.max_entries { args.push("--max-entries".into()); args.push(max_entries.to_string().into()); }
        if let Some(threads) = self.threads { args.push("--threads".into()); args.push(threads.to_string().into()); }
        args
    }
}
