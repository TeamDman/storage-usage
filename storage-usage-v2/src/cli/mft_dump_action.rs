use clap::Args;
use std::path::PathBuf;

#[derive(Args, Clone)]
pub struct MftDumpArgs {
    pub output_path: PathBuf,
}
