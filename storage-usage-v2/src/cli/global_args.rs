use clap::Args;
use std::ffi::OsString;

#[derive(Args, Default)]
pub struct GlobalArgs {
    /// Enable debug logging
    #[clap(long)]
    pub debug: bool,
}

impl GlobalArgs {
    pub fn log_level(&self) -> tracing::Level {
        if self.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        }
    }
}

impl crate::elevation_commands::ToArgs for GlobalArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, args: &mut Vec<OsString>) {
        if self.debug {
            args.push("--debug".into());
        }
    }
}
