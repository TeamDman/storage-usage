use crate::cli::mft_dump_action::MftDumpArgs;
use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct MftArgs {
    #[clap(subcommand)]
    pub action: MftAction,
}

#[derive(Subcommand, Clone)]
pub enum MftAction {
    Dump(MftDumpArgs),
}
