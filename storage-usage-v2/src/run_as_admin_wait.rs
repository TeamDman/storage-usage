use crate::to_args::Invocable;
use crate::win_strings::EasyPCWSTR;
use eyre::Context;
use std::ffi::OsString;
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;
use windows::Win32::UI::Shell::SHELLEXECUTEINFOW;
use windows::Win32::UI::Shell::ShellExecuteExW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub struct AdminChild {
    pub h_process: HANDLE,
}

impl AdminChild {
    pub fn wait(self) -> eyre::Result<u32> {
        unsafe {
            WaitForSingleObject(self.h_process, INFINITE);
            let mut code = 0u32;
            GetExitCodeProcess(self.h_process, &mut code)
                .map_err(|e| eyre::eyre!("Failed to get exit code: {}", e))?;
            CloseHandle(self.h_process)?;
            Ok(code)
        }
    }
}

pub fn run_as_admin(invocable: &impl Invocable) -> eyre::Result<AdminChild> {
    // Build a single space-separated string of arguments
    let params: OsString = invocable
        .args()
        .into_iter()
        .fold(OsString::new(), |mut acc, arg| {
            acc.push(arg);
            acc.push(" ");
            acc
        });

    // ---------------- ShellExecuteExW ----------------
    let verb = "runas".easy_pcwstr()?;
    let file = invocable.executable().easy_pcwstr()?;
    let params = params.easy_pcwstr()?;
    unsafe {
        let mut sei = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: verb.as_ptr(),
            lpFile: file.as_ptr(),
            lpParameters: params.as_ptr(),
            nShow: SW_SHOWNORMAL.0 as i32,
            ..Default::default()
        };
        ShellExecuteExW(&mut sei).wrap_err("Failed to run as administrator")?;
        Ok(AdminChild {
            h_process: sei.hProcess,
        })
    }
}
