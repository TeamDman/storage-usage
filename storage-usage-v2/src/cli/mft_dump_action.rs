use clap::Args;
use std::path::PathBuf;

#[derive(Args, Clone)]
pub struct MftDumpArgs {
    pub output_path: PathBuf,
    
    #[clap(long, help = "Overwrite existing output file")]
    pub overwrite_existing: bool,
}

impl MftDumpArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::mft_dump::dump_mft_to_file(self.output_path, self.overwrite_existing)
    }
}
