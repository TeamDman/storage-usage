use crate::cli::elevation_action::ElevationArgs;
use crate::cli::mft_action::MftArgs;
use clap::Subcommand;
use std::ffi::OsString;

#[derive(Subcommand)]
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

impl crate::elevation_commands::ToArgs for Action {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, args: &mut Vec<OsString>) {
        match self {
            Action::Mft(mft_args) => {
                args.push("mft".into());
                mft_args.add_args(args);
            }
            Action::Elevation(elevation_args) => {
                args.push("elevation".into());
                elevation_args.add_args(args);
            }
        }
    }
}
