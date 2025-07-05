use crate::cli::elevation_action::ElevationArgs;
use crate::cli::mft_action::MftArgs;
use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Subcommand;
use std::ffi::OsString;

#[derive(Subcommand, Arbitrary, PartialEq, Debug)]
pub enum Action {
    Mft(MftArgs),
    Elevation(ElevationArgs),
}

impl Action {
    pub fn run(self) -> eyre::Result<()> {
        match self {
            Action::Mft(args) => args.run(),
            Action::Elevation(args) => args.run(),
        }
    }
}

impl ToArgs for Action {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match self {
            Action::Mft(mft_args) => {
                args.push("mft".into());
                args.extend(mft_args.to_args());
            }
            Action::Elevation(elevation_args) => {
                args.push("elevation".into());
                args.extend(elevation_args.to_args());
            }
        }
        args
    }
}
