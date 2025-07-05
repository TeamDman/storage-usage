use crate::cli::mft_dump_action::MftDumpArgs;
use clap::Args;
use clap::Subcommand;
use std::ffi::OsString;

#[derive(Args)]
pub struct MftArgs {
    #[clap(subcommand)]
    pub action: MftAction,
}

impl MftArgs {
    pub fn run(self) -> eyre::Result<()> {
        self.action.run()
    }
}

impl crate::elevation_commands::ToArgs for MftArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, args: &mut Vec<OsString>) {
        self.action.add_args(args);
    }
}

#[derive(Subcommand, Clone)]
pub enum MftAction {
    Dump(MftDumpArgs),
}

impl MftAction {
    pub fn run(self) -> eyre::Result<()> {
        match self {
            MftAction::Dump(args) => args.run(),
        }
    }
}

impl crate::elevation_commands::ToArgs for MftAction {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, args: &mut Vec<OsString>) {
        match self {
            MftAction::Dump(dump_args) => {
                args.push("dump".into());
                dump_args.add_args(args);
            }
        }
    }
}
