use crate::cli::Cli;
use crate::win_elevation::is_elevated;
use crate::win_elevation::relaunch_as_admin_with_cli;
use eyre::eyre;
use std::ffi::OsString;
use tracing::info;
use tracing::warn;

/// Checks and prints the current elevation status
pub fn check_elevation() -> eyre::Result<()> {
    if is_elevated() {
        println!("Elevated");
    } else {
        println!("Not Elevated");
    }
    Ok(())
}

/// Tests the elevation relaunch procedure
pub fn test_elevation() -> eyre::Result<()> {
    if is_elevated() {
        info!("Already running as elevated, elevation test successful!");
        return Ok(());
    }

    warn!("Not elevated. Testing relaunch as administrator...");

    // Create a CLI struct for the check command
    let check_cli = create_elevation_check_cli();

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

/// Creates a CLI struct for the elevation check command
fn create_elevation_check_cli() -> Cli {
    use crate::cli::action::Action;
    use crate::cli::elevation_action::ElevationAction;
    use crate::cli::elevation_action::ElevationArgs;
    use crate::cli::elevation_action::ElevationCheckArgs;
    use crate::cli::global_args::GlobalArgs;

    Cli {
        global_args: GlobalArgs::default(),
        action: Action::Elevation(ElevationArgs {
            action: ElevationAction::Check(ElevationCheckArgs {}),
        }),
    }
}

/// Trait for converting CLI structures to arguments
pub trait ToArgs {
    fn to_args(&self) -> Vec<OsString>;
    fn add_args(&self, args: &mut Vec<OsString>);
}

impl ToArgs for Cli {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        self.add_args(&mut args);
        args
    }

    fn add_args(&self, args: &mut Vec<OsString>) {
        self.global_args.add_args(args);
        self.action.add_args(args);
    }
}
