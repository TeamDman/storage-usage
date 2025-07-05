use clap::Args;
use clap::Subcommand;
use std::ffi::OsString;

#[derive(Args)]
pub struct ElevationArgs {
    #[clap(subcommand)]
    pub action: ElevationAction,
}

impl ElevationArgs {
    pub fn run(self) -> eyre::Result<()> {
        self.action.run()
    }
}

impl crate::elevation_commands::ToArgs for ElevationArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, args: &mut Vec<OsString>) {
        self.action.add_args(args);
    }
}

#[derive(Subcommand, Clone)]
pub enum ElevationAction {
    Check(ElevationCheckArgs),
    Test(ElevationTestArgs),
}

impl ElevationAction {
    pub fn run(self) -> eyre::Result<()> {
        match self {
            ElevationAction::Check(args) => args.run(),
            ElevationAction::Test(args) => args.run(),
        }
    }
}

impl crate::elevation_commands::ToArgs for ElevationAction {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, args: &mut Vec<OsString>) {
        match self {
            ElevationAction::Check(check_args) => {
                args.push("check".into());
                check_args.add_args(args);
            }
            ElevationAction::Test(test_args) => {
                args.push("test".into());
                test_args.add_args(args);
            }
        }
    }
}

#[derive(Args, Clone)]
pub struct ElevationCheckArgs {}

impl ElevationCheckArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::elevation_commands::check_elevation()
    }
}

impl crate::elevation_commands::ToArgs for ElevationCheckArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, _args: &mut Vec<OsString>) {
        // No additional args for check command
    }
}

#[derive(Args, Clone)]
pub struct ElevationTestArgs {}

impl ElevationTestArgs {
    pub fn run(self) -> eyre::Result<()> {
        crate::elevation_commands::test_elevation()
    }
}

impl crate::elevation_commands::ToArgs for ElevationTestArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, _args: &mut Vec<OsString>) {
        // No additional args for test command
    }
}
