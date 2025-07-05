use crate::cli::action::Action;
use crate::cli::global_args::GlobalArgs;
use clap::Parser;

pub mod action;
pub mod global_args;
pub mod mft_action;
pub mod mft_dump_action;

#[derive(Parser)]
#[clap(version)]
pub struct Cli {
    #[clap(flatten)]
    pub global_args: GlobalArgs,
    #[clap(subcommand)]
    pub action: Action,
}
