use std::io::Write;

use win_elevation::is_elevated;
use win_elevation::relaunch_as_admin;
use eyre::eyre;
use win_mft_printer::get_and_print_mft_data;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

pub mod win_elevation;
pub mod win_mft_printer;
mod init;
pub mod win_strings;
pub mod win_paged_mft_reader;
pub mod win_handles;

fn ensure_elevated() -> eyre::Result<()> {
    if !is_elevated() {
        warn!("Program needs to be ran with elevated privileges.");
        info!("Relaunching as administrator");
        match relaunch_as_admin() {
            Ok(module) if module.0 as usize > 32 => {
                info!("Successfully relaunched as administrator.");
                std::process::exit(0); // Exit the current process
            }
            Ok(module) => {
                return Err(eyre!(
                    "Failed to relaunch as administrator. Error code: {}",
                    module.0
                ));
            }
            Err(e) => {
                return Err(eyre!("Failed to relaunch as administrator: {}", e));
            }
        }
    }
    Ok(())
}

fn main() -> eyre::Result<()> {
    init::init()?;
    debug!("Hi there!");

    ensure_elevated()?;
    info!("Program is running with elevated privileges.");
    let exit = match do_stuff() {
        Ok(_) => 0,
        Err(_) => 1,
    };

    info!("We have reached the end of the program.");
    wait_for_enter();
    std::process::exit(exit);
}

/// Waits for the user to press Enter.
pub fn wait_for_enter() {
    print!("Press Enter to exit...");
    std::io::stdout().flush().unwrap(); // Ensure the prompt is displayed immediately
    let _ = std::io::stdin().read_line(&mut String::new()); // Wait for user input
}

fn do_stuff() -> eyre::Result<()> {
    if let Err(e) = get_and_print_mft_data() {
        error!("Failed to get and print MFT data: {}", e);
        return Err(e);
    }
    Ok(())
}
