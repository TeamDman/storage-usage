use crate::win_strings::to_wide_null;
use byte_unit::Byte;
use byte_unit::Unit;
use byte_unit::UnitType;
use mft::MftParser;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::mem::size_of;
use std::ops::Deref;
use std::ptr::null_mut;
use tracing::debug;
use tracing::info;
use tracing::warn;
use windows::core::PCWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::CreateFileW;
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::Storage::FileSystem::SetFilePointerEx;
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::Storage::FileSystem::FILE_BEGIN;
use windows::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows::Win32::System::Ioctl::FSCTL_GET_NTFS_VOLUME_DATA;
use windows::Win32::System::Ioctl::NTFS_VOLUME_DATA_BUFFER;
use windows::Win32::System::IO::DeviceIoControl;

/// Auto-closing handle wrapper
pub struct AutoClosingHandle(HANDLE);
impl Deref for AutoClosingHandle {
    type Target = HANDLE;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for AutoClosingHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Opens a handle to the specified drive.
pub fn get_drive_handle(drive_letter: char) -> Result<AutoClosingHandle, windows::core::Error> {
    let drive_path = format!("\\\\.\\{}:", drive_letter);
    let drive_path = to_wide_null(&drive_path);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(drive_path.as_ptr()),
            FILE_GENERIC_READ.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_MODE(
                FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0,
            ),
            Some(null_mut()),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE::default(),
        )
    };

    let handle = match handle {
        Ok(handle) => handle,
        Err(err) => {
            warn!(
                "Failed to open volume handle, did you forget to elevate? -- {}",
                err
            );
            return Err(err);
        }
    };

    Ok(AutoClosingHandle(handle))
}