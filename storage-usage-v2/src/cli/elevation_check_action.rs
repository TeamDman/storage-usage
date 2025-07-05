use clap::Args;
use crate::{to_args::ToArgs, win_elevation::is_elevated};
use std::{ffi::OsString, io::Write};

#[derive(Args, Clone)]
pub struct ElevationCheckArgs {}

impl ElevationCheckArgs {
    pub fn run(self) -> eyre::Result<()> {
        if is_elevated() {
            println!("Elevated");
            println!("Press Enter to continue...");
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
        } else {
            println!("Not Elevated");
        }
        Ok(())
    }
}

impl ToArgs for ElevationCheckArgs {
    fn to_args(&self) -> Vec<OsString> {
        // No additional args for check command
        Vec::new()
    }
}
