use std::env;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{HANDLE, HWND, HMODULE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION};
use windows::Win32::Security::TOKEN_QUERY;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

/// Converts a Rust `&str` to a null-terminated wide string (`Vec<u16>`).
fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(once(0)) // Append null terminator
        .collect()
}

/// Checks if the current process is running with elevated privileges.
fn is_elevated() -> bool {
    unsafe {
        let mut token_handle = HANDLE::default();
        if !OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).as_bool() {
            eprintln!("Failed to open process token");
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut return_length = 0;

        let result = GetTokenInformation(
            token_handle,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );

        if result.as_bool() {
            elevation.TokenIsElevated != 0
        } else {
            eprintln!("Failed to get token information");
            false
        }
    }
}

/// Relaunches the current executable with administrative privileges.
fn relaunch_as_admin() -> Result<HMODULE, windows::core::Error> {
    // Get the path to the current executable
    let exe_path = env::current_exe().expect("Failed to get current executable path");
    let exe_path_str = exe_path.to_string_lossy();

    // Convert strings to wide strings
    let operation = to_wide_null("runas");
    let file = to_wide_null(&exe_path_str);
    let params = to_wide_null(""); // No parameters
    let dir = to_wide_null("");     // Current directory

    // Call ShellExecuteW
    let result = unsafe {
        ShellExecuteW(
            HWND(0),
            PCWSTR(operation.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(params.as_ptr()),
            PCWSTR(dir.as_ptr()),
            SW_SHOWNORMAL,
        )
    };

    // Check if the operation was successful
    if result.0 as usize > 32 {
        Ok(result)
    } else {
        Err(windows::core::Error::from_win32())
    }
}

/// Waits for the user to press Enter.
fn wait_for_enter() {
    print!("Press Enter to exit...");
    io::stdout().flush().unwrap(); // Ensure the prompt is displayed immediately
    let _ = io::stdin().read_line(&mut String::new()); // Wait for user input
}

fn main() {
    if is_elevated() {
        println!("Program is running with elevated privileges.");
        
        // Place your volume analysis code here.

        // Wait for Enter key before exiting
        wait_for_enter();
    } else {
        println!("Program is not elevated. Relaunching as administrator...");

        match relaunch_as_admin() {
            Ok(module) if module.0 as usize > 32 => {
                println!("Successfully relaunched as administrator.");
                std::process::exit(0); // Exit the current process
            }
            Ok(module) => {
                eprintln!("Failed to relaunch as administrator. Error code: {}", module.0);
            }
            Err(e) => {
                eprintln!("Failed to relaunch as administrator: {}", e);
            }
        }
    }
}
