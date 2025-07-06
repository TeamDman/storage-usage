use crate::cli::mft_diff_action::MftDiffArgs;
use crate::cli::mft_dump_action::MftDumpArgs;
use crate::cli::mft_query_action::MftQueryArgs;
use crate::cli::mft_summarize_action::MftSummarizeArgs;
use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use clap::Subcommand;
use std::ffi::OsString;

#[derive(Args, Arbitrary, PartialEq, Debug)]
pub struct MftArgs {
    #[clap(subcommand)]
    pub action: MftAction,
}

impl MftArgs {
    pub fn run(self) -> eyre::Result<()> {
        self.action.run()
    }
}

impl ToArgs for MftArgs {
    fn to_args(&self) -> Vec<OsString> {
        self.action.to_args()
    }
}

#[derive(Subcommand, Clone, Arbitrary, PartialEq, Debug)]
pub enum MftAction {
    Dump(MftDumpArgs),
    Diff(MftDiffArgs),
    Summarize(MftSummarizeArgs),
    Query(MftQueryArgs),
}

impl MftAction {
    pub fn run(self) -> eyre::Result<()> {
        match self {
            MftAction::Dump(args) => args.run(),
            MftAction::Diff(args) => args.run(),
            MftAction::Summarize(args) => args.run(),
            MftAction::Query(args) => args.run(),
        }
    }
}

impl ToArgs for MftAction {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match self {
            MftAction::Dump(dump_args) => {
                args.push("dump".into());
                args.extend(dump_args.to_args());
            }
            MftAction::Diff(diff_args) => {
                args.push("diff".into());
                args.extend(diff_args.to_args());
            }
            MftAction::Summarize(summarize_args) => {
                args.push("summarize".into());
                args.extend(summarize_args.to_args());
            }
            MftAction::Query(query_args) => {
                args.push("query".into());
                args.extend(query_args.to_args());
            }
        }
        args
    }
}
