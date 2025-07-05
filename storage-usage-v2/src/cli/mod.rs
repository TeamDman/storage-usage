use crate::cli::action::Action;
use crate::cli::global_args::GlobalArgs;
use crate::to_args::{Invocable, ToArgs};
use clap::Parser;
use std::ffi::OsString;

pub mod action;
pub mod elevation_action;
pub mod elevation_check_action;
pub mod elevation_test_action;
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

impl Cli {
    pub fn run(self) -> eyre::Result<()> {
        self.action.run()
    }
}

impl ToArgs for Cli {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.extend(self.global_args.to_args());
        args.extend(self.action.to_args());
        args
    }
}

impl Invocable for Cli {
    fn executable(&self) -> std::path::PathBuf {
        std::env::current_exe().expect("Failed to get current executable path")
    }
    
    fn args(&self) -> Vec<OsString> {
        self.to_args()
    }
}
