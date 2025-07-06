use crate::win_elevation::is_elevated;
use crate::win_elevation::relaunch_as_admin;
use crate::win_handles::get_drive_handle;
use eyre::Context;
use eyre::eyre;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::mem::size_of;
use std::path::Path;
use tracing::info;
use tracing::warn;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::LUID;
use windows::Win32::Security::AdjustTokenPrivileges;
use windows::Win32::Security::LookupPrivilegeValueW;
use windows::Win32::Security::SE_BACKUP_NAME;
use windows::Win32::Security::SE_PRIVILEGE_ENABLED;
use windows::Win32::Security::SE_RESTORE_NAME;
use windows::Win32::Security::SE_SECURITY_NAME;
use windows::Win32::Security::TOKEN_ADJUST_PRIVILEGES;
use windows::Win32::Security::TOKEN_PRIVILEGES;
use windows::Win32::Security::TOKEN_QUERY;
use windows::Win32::Storage::FileSystem::CreateFileW;
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::Storage::FileSystem::FILE_BEGIN;
use windows::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::Storage::FileSystem::SetFilePointerEx;
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::FSCTL_GET_NTFS_VOLUME_DATA;
use windows::Win32::System::Ioctl::NTFS_VOLUME_DATA_BUFFER;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::System::Threading::OpenProcessToken;

/// Dumps the MFT to the specified file path
pub fn dump_mft_to_file<P: AsRef<Path>>(
    output_path: P,
    overwrite_existing: bool,
    drive_letter: char,
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

    // Enable backup privileges to access system files like $MFT
    enable_backup_privileges()
        .with_context(|| "Failed to enable backup privileges")?;

    // Use the provided drive letter
    let drive_letter = drive_letter.to_uppercase().next().unwrap_or('C');

    // Validate that the drive is using NTFS filesystem
    info!("Validating filesystem type for drive {}...", drive_letter);
    validate_ntfs_filesystem(drive_letter)
        .with_context(|| format!("NTFS validation failed for drive {}", drive_letter))?;

    info!("Reading MFT data from drive {}...", drive_letter);
    let mft_data = read_mft_data(drive_letter)?;

    info!("Writing MFT data to '{}'...", output_path.display());
    write_mft_to_file(&mft_data, output_path)?;

    info!(
        "Successfully dumped MFT ({}) to '{}'",
        humansize::format_size(mft_data.len(), humansize::DECIMAL),
        output_path.display()
    );

    Ok(())
}

/// Validates that the specified drive is using NTFS filesystem
fn validate_ntfs_filesystem(drive_letter: char) -> eyre::Result<()> {
    // For now, we'll validate by attempting to get NTFS volume data
    // If this succeeds, we know it's an NTFS volume
    let drive_handle = get_drive_handle(drive_letter)
        .with_context(|| format!("Failed to open handle to drive {drive_letter}"))?;

    let mut volume_data = NTFS_VOLUME_DATA_BUFFER::default();
    let mut bytes_returned = 0u32;

    let result = unsafe {
        DeviceIoControl(
            *drive_handle,
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
            0,
            Some(&mut volume_data as *mut _ as *mut _),
            size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32,
            Some(&mut bytes_returned),
            None,
        )
    };

    match result {
        Ok(_) => {
            info!("✓ Filesystem validation passed: Drive {} is using NTFS", drive_letter);
            info!("NTFS Volume Info:");
            // info!("  VolumeSerialNumber: 0x{:X}", volume_data.VolumeSerialNumber);
            info!("  NumberSectors: {}", volume_data.NumberSectors);
            info!("  TotalClusters: {}", volume_data.TotalClusters);
            info!("  FreeClusters: {}", volume_data.FreeClusters);
            info!("  BytesPerSector: {}", volume_data.BytesPerSector);
            info!("  BytesPerCluster: {}", volume_data.BytesPerCluster);
            Ok(())
        }
        Err(e) => {
            Err(eyre!(
                "Drive {} does not appear to be using NTFS filesystem. FSCTL_GET_NTFS_VOLUME_DATA failed: {}. MFT dumping is only supported on NTFS volumes.",
                drive_letter, e
            ))
        }
    }
}

/// Reads the raw MFT data by opening the $MFT file directly  
fn read_mft_data(drive_letter: char) -> eyre::Result<Vec<u8>> {
    // Try the simple approach first - open $MFT as a file with backup semantics
    let path = format!(r"\\.\{}:\$MFT", drive_letter);
    
    info!("Attempting to open $MFT file: {}", path);
    
    // Use CreateFileW directly for better control over access flags
    let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    
    let handle_result = unsafe {
        CreateFileW(
            windows::core::PCWSTR::from_raw(path_wide.as_ptr()),
            FILE_GENERIC_READ.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_MODE(
                FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0,
            ),
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };
    
    match handle_result {
        Ok(handle) => {
            info!("Successfully opened $MFT file directly");
            return read_mft_from_handle(handle);
        }
        Err(e) => {
            warn!("Failed to open $MFT file directly: {}, falling back to volume approach", e);
        }
    }
    
    // Fallback: Use volume access approach but read only the first MFT segment
    // This won't get the full MFT if it's fragmented, but will get the core records
    info!("Using fallback volume access approach");
    read_mft_from_volume(drive_letter)
}

fn read_mft_from_handle(handle: HANDLE) -> eyre::Result<Vec<u8>> {
    // Ensure handle gets closed
    let _handle_guard = HandleGuard(handle);
    
    // Get file size
    let file_size = unsafe {
        let mut file_size = 0i64;
        windows::Win32::Storage::FileSystem::GetFileSizeEx(handle, &mut file_size)
            .with_context(|| "Failed to get $MFT file size")?;
        file_size as usize
    };
    
    info!(
        "MFT file size: {}",
        humansize::format_size(file_size, humansize::DECIMAL)
    );
    
    // Read the entire file
    let mut mft_data = vec![0u8; file_size];
    let mut total_bytes_read = 0;
    let mut offset = 0;
    
    while offset < file_size {
        let remaining = file_size - offset;
        let chunk_size = remaining.min(1024 * 1024); // Read in 1MB chunks
        
        let mut bytes_read = 0u32;
        unsafe {
            ReadFile(
                handle,
                Some(&mut mft_data[offset..offset + chunk_size]),
                Some(&mut bytes_read),
                None,
            )
            .with_context(|| format!("Failed to read MFT data at offset {}", offset))?;
        }
        
        if bytes_read == 0 {
            break; // EOF
        }
        
        offset += bytes_read as usize;
        total_bytes_read += bytes_read as usize;
    }
    
    mft_data.truncate(total_bytes_read);
    
    info!(
        "Successfully read MFT data: {}",
        humansize::format_size(total_bytes_read, humansize::DECIMAL)
    );
    
    Ok(mft_data)
}

fn read_mft_from_volume(drive_letter: char) -> eyre::Result<Vec<u8>> {
    // Get a handle to the volume
    let drive_handle = get_drive_handle(drive_letter)
        .with_context(|| format!("Failed to open handle to drive {drive_letter}"))?;

    // Get NTFS volume data to locate the MFT
    let mut volume_data = NTFS_VOLUME_DATA_BUFFER::default();
    let mut bytes_returned = 0u32;

    unsafe {
        DeviceIoControl(
            *drive_handle,
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
            0,
            Some(&mut volume_data as *mut _ as *mut _),
            size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .with_context(|| "Failed to get NTFS volume data")?;
    }

    info!("MFT starts at LCN: {}", volume_data.MftStartLcn);
    info!(
        "MFT valid data length: {}",
        humansize::format_size_i(volume_data.MftValidDataLength, humansize::DECIMAL)
    );
    info!("Bytes per cluster: {}", volume_data.BytesPerCluster);

    let mft_start_offset = volume_data.MftStartLcn as u64 * volume_data.BytesPerCluster as u64;
    
    // Read only the first 16MB of MFT to avoid fragmentation issues
    // This should contain most of the core system files and metadata
    let max_read_size = 16 * 1024 * 1024; // 16MB
    let actual_read_size = (volume_data.MftValidDataLength as usize).min(max_read_size);
    
    warn!(
        "Reading only the first {} of MFT data to avoid fragmentation issues",
        humansize::format_size(actual_read_size, humansize::DECIMAL)
    );
    
    let mut mft_data = vec![0u8; actual_read_size];

    // Seek to MFT start
    unsafe {
        SetFilePointerEx(*drive_handle, mft_start_offset as i64, None, FILE_BEGIN)
            .with_context(|| "Failed to seek to MFT start")?;
    }

    // Read MFT data
    let mut bytes_read = 0u32;
    unsafe {
        ReadFile(
            *drive_handle,
            Some(mft_data.as_mut_slice()),
            Some(&mut bytes_read),
            None,
        )
        .with_context(|| "Failed to read MFT data")?;
    }

    if bytes_read as usize != mft_data.len() {
        warn!(
            "Expected to read {} bytes, but only read {} bytes",
            mft_data.len(),
            bytes_read
        );
        mft_data.truncate(bytes_read as usize);
    }

    info!(
        "Successfully read MFT data: {}",
        humansize::format_size(bytes_read as usize, humansize::DECIMAL)
    );

    Ok(mft_data)
}

/// RAII guard for Windows HANDLE
struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
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

/// Enables backup and security privileges for the current process
fn enable_backup_privileges() -> eyre::Result<()> {
    use std::mem::size_of;
    
    unsafe {
        // Get current process token
        let mut token = windows::Win32::Foundation::HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .with_context(|| "Failed to open process token")?;

        // Enable multiple privileges that might be needed
        let privileges_to_enable = [
            SE_BACKUP_NAME,
            SE_RESTORE_NAME,
            SE_SECURITY_NAME,
        ];

        for privilege_name in &privileges_to_enable {
            // Look up the privilege LUID
            let mut luid = LUID::default();
            if LookupPrivilegeValueW(None, *privilege_name, &mut luid).is_ok() {
                // Set up the privilege structure
                let mut privileges = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [windows::Win32::Security::LUID_AND_ATTRIBUTES {
                        Luid: luid,
                        Attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };

                // Adjust token privileges
                let _ = AdjustTokenPrivileges(
                    token,
                    false,
                    Some(&mut privileges),
                    size_of::<TOKEN_PRIVILEGES>() as u32,
                    None,
                    None,
                );
            }
        }

        // Close token handle
        windows::Win32::Foundation::CloseHandle(token)
            .with_context(|| "Failed to close token handle")?;

        info!("Successfully enabled backup privileges");
        Ok(())
    }
}
