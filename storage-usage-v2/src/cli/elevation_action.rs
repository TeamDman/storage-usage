use crate::cli::elevation_check_action::ElevationCheckArgs;
use crate::cli::elevation_test_action::ElevationTestArgs;
use crate::to_args::ToArgs;
use clap::Args;
use clap::Subcommand;
use std::ffi::OsString;

#[derive(Args)]
pub struct ElevationArgs {
    #[clap(subcommand)]
    pub action: ElevationAction,
}

impl ElevationArgs {
    pub fn run(self) -> eyre::Result<()> {
        self.action.run()
    }
}

impl ToArgs for ElevationArgs {
    fn to_args(&self) -> Vec<OsString> {
        self.action.to_args()
    }
}

#[derive(Subcommand, Clone)]
pub enum ElevationAction {
    Check(ElevationCheckArgs),
    Test(ElevationTestArgs),
}

impl ElevationAction {
    pub fn run(self) -> eyre::Result<()> {
        match self {
            ElevationAction::Check(args) => args.run(),
            ElevationAction::Test(args) => args.run(),
        }
    }
}

impl ToArgs for ElevationAction {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match self {
            ElevationAction::Check(check_args) => {
                args.push("check".into());
                args.extend(check_args.to_args());
            }
            ElevationAction::Test(test_args) => {
                args.push("test".into());
                args.extend(test_args.to_args());
            }
        }
        args
    }
}
