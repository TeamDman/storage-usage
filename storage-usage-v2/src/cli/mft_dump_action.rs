use clap::Args;
use std::ffi::OsString;
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

impl crate::elevation_commands::ToArgs for MftDumpArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, args: &mut Vec<OsString>) {
        args.push(self.output_path.as_os_str().into());
        if self.overwrite_existing {
            args.push("--overwrite-existing".into());
        }
    }
}
