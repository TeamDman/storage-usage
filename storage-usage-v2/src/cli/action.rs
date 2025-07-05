use crate::cli::mft_action::MftArgs;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Action {
    Mft(MftArgs),
}
