use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use std::ffi::OsString;
use super::drive_letter_pattern::DriveLetterPattern;
use std::time::Duration;
use humantime::parse_duration;

/// Arguments for fuzzy searching files within cached MFTs matching a drive pattern
#[derive(Args, Clone, PartialEq, Debug, Arbitrary)]
pub struct MftQueryArgs {
    #[clap(
        long,
        help = "Drive letter pattern to select cached MFTs (e.g. '*', 'C', 'CD', 'C,D')",
        default_value_t = DriveLetterPattern::default()
    )]
    pub drive_pattern: DriveLetterPattern,

    #[clap(help = "Search query for fuzzy matching filenames")]
    pub query: String,

    #[clap(
        long,
        default_value = "100",
        help = "Maximum number of results to display"
    )]
    pub limit: usize,

    #[clap(
        long = "display-interval",
        default_value = "1s",
        value_parser = parse_duration,
        help = "Interval to re-display the top results during ongoing collection (e.g. '500ms', '2s')"
    )]
    pub display_interval: Duration,

    #[clap(
        long = "top",
        default_value = "10",
        help = "Number of top matches to show each interval"
    )]
    pub top_n: usize,
}

impl MftQueryArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::mft_query::query_mft_files_fuzzy(
            self.drive_pattern,
            self.query,
            self.limit,
            self.display_interval,
            self.top_n,
        )
    }
}

impl ToArgs for MftQueryArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.drive_pattern.to_string().into());
        args.push(self.query.clone().into());
        if self.limit != 100 {
            args.push("--limit".into());
            args.push(self.limit.to_string().into());
        }
        if self.display_interval != Duration::from_secs(1) {
            args.push("--display-interval".into());
            args.push(humantime::format_duration(self.display_interval).to_string().into());
        }
        if self.top_n != 10 {
            args.push("--top".into());
            args.push(self.top_n.to_string().into());
        }
        args
    }
}
