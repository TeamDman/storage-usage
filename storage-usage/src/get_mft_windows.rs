use byte_unit::Byte;
use byte_unit::Unit;
use byte_unit::UnitType;
use mft::MftParser;
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

use crate::strings::to_wide_null;

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

pub fn get_mft_buffer(
    drive_handle: HANDLE,
) -> eyre::Result<NTFS_VOLUME_DATA_BUFFER, windows::core::Error> {
    let mut volume_data = NTFS_VOLUME_DATA_BUFFER::default();
    let mut bytes_read = 0;

    unsafe {
        DeviceIoControl(
            drive_handle,
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
            0,
            Some(&mut volume_data as *mut _ as *mut _),
            size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32,
            Some(&mut bytes_read),
            None,
        )
        .ok()?
    }
    debug!("Read {bytes_read} bytes of NTFS volume metadata");
    Ok(volume_data)
}

pub fn get_and_print_mft_data() -> eyre::Result<()> {
    let drive_handle = get_drive_handle('C')?;
    let volume_data = get_mft_buffer(*drive_handle)?;
    debug!("Volume data: {:#?}", volume_data);

    let bytes_per_cluster = volume_data.BytesPerCluster as u64;
    let mft_start_offset = volume_data.MftStartLcn as u64 * bytes_per_cluster;
    let mft_valid_data_length = volume_data.MftValidDataLength as u64;

    // Set a maximum MFT size to read (e.g., 10 MB for testing)
    let max_mft_size = Byte::from_u64_with_unit(1, Unit::GiB).unwrap();
    let mft_read_size = std::cmp::min(mft_valid_data_length as usize, max_mft_size.as_u64() as usize);

    debug!("Reading {} bytes from MFT", mft_read_size);

    // Seek to the MFT start offset
    unsafe {
        SetFilePointerEx(*drive_handle, mft_start_offset as i64, None, FILE_BEGIN).ok()?;
    }

    // Read MFT data into a buffer
    let mut mft_data = vec![0u8; mft_read_size];
    let mut bytes_read = 0u32;
    unsafe {
        ReadFile(
            *drive_handle,
            Some(mft_data.as_mut_ptr() as *mut _),
            mft_read_size as u32,
            Some(&mut bytes_read),
            None,
        )
        .ok()?;
    }

    // Truncate buffer to actual bytes read
    mft_data.truncate(bytes_read as usize);

    debug!("Read {} bytes from MFT", bytes_read);

    // Now, feed mft_data into MftParser
    let mut parser = MftParser::from_buffer(mft_data)?;

    // Iterate over entries
    let mut invalid_count = 0;
    let mut total_logical = 0;
    let mut total_physical = 0;
    let mut total_entries = 0;
    for (i, entry) in parser.iter_entries().enumerate() {
        total_entries += 1;
        let should_log = i < 100 || i % 100 == 0;
        match entry {
            Ok(e) if !e.header.is_valid() => {
                invalid_count += 1;
            }
            Ok(e) => match e.find_best_name_attribute() {
                Some(x) => {
                    if e.is_dir() {
                        if should_log {
                            info!("Found dir {}", x.name);
                        }
                    } else {
                        total_logical += x.logical_size;
                        total_physical += x.physical_size;
                        if should_log {
                            info!(
                                "Found {} (physical={:#}, logical={:#})",
                                x.name,
                                Byte::from_u64(x.physical_size),
                                Byte::from_u64(x.logical_size)
                            );
                        }
                    }
                }
                None => {
                    // warn!("Bruh {:?}", e.header);
                }
            },
            Err(err) => {
                eprintln!("Error reading entry: {}", err);
            }
        }
    }
    if invalid_count > 0 {
        warn!("Found {} invalid entries", invalid_count);
    }
    info!("Total entries: {}", total_entries);
    info!("Total logical size: {:#.2}", Byte::from_u64(total_logical).get_appropriate_unit(UnitType::Binary));
    info!("Total physical size: {:#.2}", Byte::from_u64(total_physical).get_appropriate_unit(UnitType::Binary));

    Ok(())
}
