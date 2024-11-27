use std::env;
use std::mem::size_of;
use windows::core::PCWSTR;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::HWND;
use windows::Win32::Security::GetTokenInformation;
use windows::Win32::Security::TokenElevation;
use windows::Win32::Security::TOKEN_ELEVATION;
use windows::Win32::Security::TOKEN_QUERY;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::System::Threading::OpenProcessToken;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::strings::to_wide_null;

/// Checks if the current process is running with elevated privileges.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token_handle = HANDLE::default();
        if !OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).as_bool() {
            eprintln!("Failed to open process token. Error: {:?}", GetLastError());
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut return_length = 0;

        let result = GetTokenInformation(
            token_handle,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );

        if result.as_bool() {
            elevation.TokenIsElevated != 0
        } else {
            eprintln!(
                "Failed to get token information. Error: {:?}",
                GetLastError()
            );
            false
        }
    }
}

/// Relaunches the current executable with administrative privileges.
pub fn relaunch_as_admin() -> Result<windows::Win32::Foundation::HMODULE, windows::core::Error> {
    // Get the path to the current executable
    let exe_path = env::current_exe().expect("Failed to get current executable path");
    let exe_path_str = exe_path.to_string_lossy();

    // Convert strings to wide strings
    let operation = to_wide_null("runas");
    let file = to_wide_null(&exe_path_str);
    let params = to_wide_null(""); // No parameters
    let dir = to_wide_null(""); // Current directory

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
