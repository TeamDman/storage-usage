use clap::Args;
use crate::to_args::ToArgs;
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

impl ToArgs for GlobalArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        if self.debug {
            args.push("--debug".into());
        }
        args
    }
}
