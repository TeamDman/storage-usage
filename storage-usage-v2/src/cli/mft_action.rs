use crate::cli::mft_dump_action::MftDumpArgs;
use clap::Args;
use clap::Subcommand;

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
