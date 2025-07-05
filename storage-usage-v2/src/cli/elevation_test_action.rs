use crate::cli::Cli;
use crate::cli::action::Action;
use crate::cli::elevation_action::ElevationAction;
use crate::cli::elevation_action::ElevationArgs;
use crate::cli::elevation_check_action::ElevationCheckArgs;
use crate::cli::global_args::GlobalArgs;
use crate::to_args::ToArgs;
use crate::win_elevation::is_elevated;
use crate::win_elevation::relaunch_as_admin_with_cli;
use clap::Args;
use eyre::eyre;
use std::ffi::OsString;
use tracing::info;
use tracing::warn;

#[derive(Args, Clone)]
pub struct ElevationTestArgs {}

impl ElevationTestArgs {
    pub fn run(self) -> eyre::Result<()> {
        if is_elevated() {
            info!("Already running as elevated, elevation test successful!");
            return Ok(());
        }

        warn!("Not elevated. Testing relaunch as administrator...");

        // Create a CLI struct for the check command
        let check_cli = {
            Cli {
                global_args: GlobalArgs::default(),
                action: Action::Elevation(ElevationArgs {
                    action: ElevationAction::Check(ElevationCheckArgs {}),
                }),
            }
        };

        info!("Relaunching as administrator to run elevation check...");
        match relaunch_as_admin_with_cli(&check_cli) {
            Ok(module) if module.0 as usize > 32 => {
                info!("Successfully relaunched as administrator for elevation test.");
                std::process::exit(0); // Exit the current process
            }
            Ok(module) => {
                return Err(eyre!(
                    "Failed to relaunch as administrator. Error code: {:?}",
                    module.0 as usize
                ));
            }
            Err(e) => {
                return Err(eyre!("Failed to relaunch as administrator: {}", e));
            }
        }
    }
}

impl ToArgs for ElevationTestArgs {
    fn to_args(&self) -> Vec<OsString> {
        // No additional args for test command
        Vec::new()
    }
}
