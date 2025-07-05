use crate::to_args::ToArgs;
use clap::Args;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Args, Clone)]
pub struct MftDumpArgs {
    pub output_path: PathBuf,

    #[clap(long, help = "Overwrite existing output file")]
    pub overwrite_existing: bool,
    
    #[clap(long, short = 'd', default_value = "C", help = "Drive letter to dump MFT from")]
    pub drive_letter: char,
}

impl MftDumpArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::mft_dump::dump_mft_to_file(self.output_path, self.overwrite_existing, self.drive_letter)
    }
}

impl ToArgs for MftDumpArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.output_path.as_os_str().into());
        if self.overwrite_existing {
            args.push("--overwrite-existing".into());
        }
        args.push("--drive-letter".into());
        args.push(self.drive_letter.to_string().into());
        args
    }
}
