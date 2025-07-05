use crate::cli::mft_action::MftArgs;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Action {
    Mft(MftArgs),
}

impl Action {
    pub fn run(self) -> eyre::Result<()> {
        match self {
            Action::Mft(args) => args.run(),
        }
    }
}
