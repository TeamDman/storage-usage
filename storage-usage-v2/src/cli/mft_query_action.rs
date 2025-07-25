use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use std::ffi::OsString;
use std::path::PathBuf;

/// Arguments for fuzzy searching files within an MFT
#[derive(Args, Clone, PartialEq, Debug, Arbitrary)]
pub struct MftQueryArgs {
    #[clap(help = "Path to the MFT file to query")]
    pub mft_file: PathBuf,

    #[clap(help = "Search query for fuzzy matching filenames")]
    pub query: String,

    #[clap(
        long,
        default_value = "100",
        help = "Maximum number of results to display"
    )]
    pub limit: usize,
}

impl MftQueryArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::mft_query::query_mft_files_fuzzy(
            self.mft_file,
            self.query,
            self.limit,
        )
    }
}

impl ToArgs for MftQueryArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.mft_file.as_os_str().into());

        // Add query
        args.push(self.query.clone().into());

        if self.limit != 100 {
            args.push("--limit".into());
            args.push(self.limit.to_string().into());
        }

        args
    }
}
