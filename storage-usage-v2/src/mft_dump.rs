use crate::win_elevation::is_elevated;
use crate::win_elevation::relaunch_as_admin;
use crate::win_handles::get_drive_handle;
use eyre::Context;
use eyre::ContextCompat;
use eyre::eyre;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::mem::size_of;
use std::path::Path;
use tracing::info;
use tracing::warn;
use windows::Win32::Storage::FileSystem::FILE_BEGIN;
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::Storage::FileSystem::SetFilePointerEx;
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::FSCTL_GET_NTFS_VOLUME_DATA;
use windows::Win32::System::Ioctl::NTFS_VOLUME_DATA_BUFFER;

/// Dumps the MFT to the specified file path
pub fn dump_mft_to_file<P: AsRef<Path>>(
    output_path: P,
    overwrite_existing: bool,
) -> eyre::Result<()> {
    let output_path = output_path.as_ref();

    // Check if file exists and handle overwrite logic
    if output_path.exists() && !overwrite_existing {
        return Err(eyre!(
            "Output file '{}' already exists. Use --overwrite-existing to overwrite it.",
            output_path.display()
        ));
    }

    // Check if we're elevated, and relaunch if not
    if !is_elevated() {
        warn!("Program needs to be run with elevated privileges.");
        info!("Relaunching as administrator...");

        match relaunch_as_admin() {
            Ok(child) => {
                info!("Spawned elevated process for MFT dump – waiting for it to finish…");
                let exit_code = child.wait()?;
                info!("Elevated MFT dump process exited with code {exit_code}");
                std::process::exit(exit_code as i32);
            }
            Err(e) => {
                return Err(eyre!("Failed to relaunch as administrator: {}", e));
            }
        }
    }

    info!("Program is running with elevated privileges.");

    // Extract drive letter from output path or default to C:
    let drive_letter = extract_drive_letter_from_path(output_path).unwrap_or('C');

    info!("Opening handle to drive {}:", drive_letter);
    let drive_handle = get_drive_handle(drive_letter)
        .with_context(|| format!("Failed to open handle to drive {}", drive_letter))?;

    info!("Reading MFT data from drive {}...", drive_letter);
    let mft_data = read_mft_data(*drive_handle)?;

    info!("Writing MFT data to '{}'...", output_path.display());
    write_mft_to_file(&mft_data, output_path)?;

    info!(
        "Successfully dumped MFT ({} bytes) to '{}'",
        mft_data.len(),
        output_path.display()
    );

    Ok(())
}

/// Extracts the drive letter from a file path
fn extract_drive_letter_from_path(path: &Path) -> Option<char> {
    path.components().next().and_then(|component| {
        if let std::path::Component::Prefix(prefix) = component {
            match prefix.kind() {
                std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter) => {
                    Some(letter as char)
                }
                _ => None,
            }
        } else {
            None
        }
    })
}

/// Reads the raw MFT data from the drive handle
fn read_mft_data(drive_handle: windows::Win32::Foundation::HANDLE) -> eyre::Result<Vec<u8>> {
    // This is a simplified implementation - in a real MFT parser, you would:
    // 1. Get NTFS volume data to find MFT location and size
    // 2. Read the MFT using proper sector alignment
    // 3. Handle MFT fragmentation

    // For now, we'll implement a basic version that demonstrates the concept

    // Get NTFS volume data to locate the MFT
    let mut volume_data = NTFS_VOLUME_DATA_BUFFER::default();
    let mut bytes_returned = 0u32;

    unsafe {
        DeviceIoControl(
            drive_handle,
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
            0,
            Some(&mut volume_data as *mut _ as *mut _),
            size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .ok()
        .context("Failed to get NTFS volume data")?;
    }

    info!("MFT starts at LCN: {}", volume_data.MftStartLcn);
    info!(
        "MFT valid data length: {} bytes",
        volume_data.MftValidDataLength
    );
    info!("Bytes per cluster: {}", volume_data.BytesPerCluster);

    let mft_start_offset = volume_data.MftStartLcn as u64 * volume_data.BytesPerCluster as u64;
    let mft_size = volume_data.MftValidDataLength as u64;

    // Read the MFT data
    let mut mft_data = vec![0u8; mft_size as usize];

    // Seek to MFT start
    unsafe {
        SetFilePointerEx(drive_handle, mft_start_offset as i64, None, FILE_BEGIN)
            .ok()
            .context("Failed to seek to MFT start")?;
    }

    // Read MFT data
    let mut bytes_read = 0u32;
    unsafe {
        ReadFile(
            drive_handle,
            Some(mft_data.as_mut_slice()),
            Some(&mut bytes_read),
            None,
        )
        .ok()
        .context("Failed to read MFT data")?;
    }

    if bytes_read as usize != mft_data.len() {
        warn!(
            "Expected to read {} bytes, but only read {} bytes",
            mft_data.len(),
            bytes_read
        );
        mft_data.truncate(bytes_read as usize);
    }

    Ok(mft_data)
}

/// Writes the MFT data to the specified file
fn write_mft_to_file(mft_data: &[u8], output_path: &Path) -> eyre::Result<()> {
    let mut file = if output_path.exists() {
        // If file exists and we got here, overwrite_existing must be true
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(output_path)
            .with_context(|| {
                format!("Failed to open file for writing: {}", output_path.display())
            })?
    } else {
        // Create new file
        File::create(output_path)
            .with_context(|| format!("Failed to create file: {}", output_path.display()))?
    };

    file.write_all(mft_data).with_context(|| {
        format!(
            "Failed to write MFT data to file: {}",
            output_path.display()
        )
    })?;

    file.flush()
        .with_context(|| format!("Failed to flush file: {}", output_path.display()))?;

    Ok(())
}
