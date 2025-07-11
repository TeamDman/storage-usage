use crate::cli::elevation_check_action::ElevationCheckArgs;
use crate::cli::elevation_test_action::ElevationTestArgs;
use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use clap::Subcommand;
use std::ffi::OsString;

/// Elevation command arguments container
#[derive(Args, Arbitrary, PartialEq, Debug)]
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

/// Administrative privilege operations
#[derive(Subcommand, Clone, Arbitrary, PartialEq, Debug)]
pub enum ElevationAction {
    /// Check if the current process is running with administrator privileges
    Check(ElevationCheckArgs),
    /// Test elevation functionality by relaunching with administrator privileges
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
