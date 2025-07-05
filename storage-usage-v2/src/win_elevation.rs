use crate::to_args::Invocable;
use crate::win_strings::EasyPCWSTR;
use std::ffi::OsString;
use std::mem::size_of;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::Foundation::HWND;
use windows::Win32::Security::GetTokenInformation;
use windows::Win32::Security::TOKEN_ELEVATION;
use windows::Win32::Security::TOKEN_QUERY;
use windows::Win32::Security::TokenElevation;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::System::Threading::OpenProcessToken;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Checks if the current process is running with elevated privileges.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token_handle = HANDLE::default();
        if !OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).is_ok() {
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

        if result.is_ok() {
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

/// Relaunches the current executable with administrative privileges, preserving arguments.
pub fn relaunch_as_admin() -> eyre::Result<HINSTANCE> {
    run_as_admin(&crate::to_args::ThisInvocation)
}

/// Runs an invocable with administrative privileges using ShellExecuteW.
pub fn run_as_admin(invocable: &impl Invocable) -> eyre::Result<HINSTANCE> {
    // Call ShellExecuteW
    let result = unsafe {
        ShellExecuteW(
            Some(HWND(std::ptr::null_mut())),
            "runas".easy_pcwstr()?.as_ref(),
            invocable.executable().easy_pcwstr()?.as_ref(),
            invocable
                .args()
                .into_iter()
                .fold(OsString::new(), |mut acc, arg| {
                    acc.push(arg);
                    acc.push(" ");
                    acc
                })
                .easy_pcwstr()?
                .as_ref(),
            "".easy_pcwstr()?.as_ref(),
            SW_SHOWNORMAL,
        )
    };

    // Check if the operation was successful
    if result.0 as usize > 32 {
        Ok(result)
    } else {
        Err(windows::core::Error::from_win32().into())
    }
}

/// Relaunches the current executable with administrative privileges using a specific CLI configuration.
pub fn relaunch_as_admin_with_cli(cli: &crate::cli::Cli) -> eyre::Result<HINSTANCE> {
    run_as_admin(cli)
}
