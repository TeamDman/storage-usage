use std::env;
use std::ffi::OsStr;
use std::io::Write;
use std::io::{self};
use std::iter::once;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use windows::core::PCWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::HWND;
use windows::Win32::Security::GetTokenInformation;
use windows::Win32::Security::TokenElevation;
use windows::Win32::Security::TOKEN_ELEVATION;
use windows::Win32::Security::TOKEN_QUERY;
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
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::System::Threading::OpenProcessToken;
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

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
fn relaunch_as_admin() -> Result<windows::Win32::Foundation::HMODULE, windows::core::Error> {
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

/// Structure representing an MFT Entry (simplified for demonstration).
#[repr(C)]
#[derive(Default)]
struct MftEntry {
    signature: [u8; 4], // Should be "FILE"
    fixup_offset: u16,
    fixup_size: u16,
    log_file_sequence_number: u64,
    sequence_number: u16,
    hard_link_count: u16,
    first_attribute_offset: u16,
    flags: u16,
    used_size: u32,
    allocated_size: u32,
    file_reference_to_base: u64,
    next_attribute_id: u16,
    padding: [u8; 2],
    // Attributes would follow, but are omitted for simplicity
}


/// Retrieves NTFS volume data.
fn get_ntfs_volume_data(handle: HANDLE) -> Option<NTFS_VOLUME_DATA_BUFFER> {
    let mut ntfs_volume_data = NTFS_VOLUME_DATA_BUFFER::default();
    let mut bytes_returned = 0u32;

    let result = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_GET_NTFS_VOLUME_DATA,
            Some(null_mut()),
            0,
            Some(&mut ntfs_volume_data as *mut _ as *mut _),
            size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32,
            Some(&mut bytes_returned),
            None,
        )
    };

    if result.as_bool() {
        Some(ntfs_volume_data)
    } else {
        eprintln!("Failed to get NTFS volume data. Error: {:?}", unsafe {
            GetLastError()
        });
        None
    }
}

/// Reads raw data from a specific cluster.
fn read_raw_cluster(handle: HANDLE, cluster: u64, cluster_size: usize) -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; cluster_size];
    let offset = cluster * cluster_size as u64;

    // Move the file pointer to the desired offset
    let success = unsafe { SetFilePointerEx(handle, offset as i64, None, FILE_BEGIN).as_bool() };

    if !success {
        eprintln!("Failed to set file pointer. Error: {:?}", unsafe {
            GetLastError()
        });
        return None;
    }

    let mut bytes_read = 0u32;
    let read_result = unsafe {
        ReadFile(
            handle,
            Some(buffer.as_mut_ptr() as *mut _),
            buffer.len() as u32,
            Some(&mut bytes_read),
            None,
        )
    };

    if read_result.as_bool() {
        buffer.truncate(bytes_read as usize);
        Some(buffer)
    } else {
        eprintln!("Failed to read raw cluster data. Error: {:?}", unsafe {
            GetLastError()
        });
        None
    }
}

/// Parses MFT entries and prints their disk usage.
fn parse_and_print_mft_entries(data: &[u8], count: usize, entry_size: usize) {
    for i in 0..count {
        let offset = i * entry_size;
        if offset + entry_size > data.len() {
            eprintln!("Insufficient data for MFT entry {}", i + 1);
            break;
        }

        let entry_data = &data[offset..offset + entry_size];
        println!("Interpreting entry data at offset {}...", offset); // error: process didn't exit successfully: `target\debug\examples\mft.exe` (exit code: 0xc0000005, STATUS_ACCESS_VIOLATION)
        let entry = unsafe { &*(entry_data.as_ptr() as *const MftEntry) };

        // Verify MFT entry signature
        if &entry.signature != b"FILE" {
            eprintln!("Invalid MFT entry signature at entry {}", i + 1);
            continue;
        }

        println!(
            "Entry {}: Used Size = {} bytes, Allocated Size = {} bytes",
            i + 1,
            entry.used_size,
            entry.allocated_size
        );
    }
}

fn main() {
    // Check if elevated, if not, relaunch as admin
    if is_elevated() {
        println!("Program is running with elevated privileges.");

        // Open the C: volume
        let drive = to_wide_null("\\\\.\\C:");
        let handle = unsafe {
            CreateFileW(
                PCWSTR(drive.as_ptr()),
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
                eprintln!(
                    "Failed to open volume handle, did you forget to elevate? -- {}",
                    err
                );
                return;
            }
        };

        // Retrieve NTFS volume data
        if let Some(volume_data) = get_ntfs_volume_data(handle) {
            let bytes_per_cluster = volume_data.BytesPerCluster;
            let mft_start = volume_data.MftStartLcn as u64; // Cast from i64 to u64

            // Calculate MFT record size based on ClustersPerFileRecordSegment
            let mft_record_size = if (volume_data.ClustersPerFileRecordSegment as i32) < 0 {
                2_u64.pow((-(volume_data.ClustersPerFileRecordSegment as i32)) as u32)
            } else {
                (volume_data.ClustersPerFileRecordSegment as u64) * (bytes_per_cluster as u64)
            };
            println!("MFT Record Size: {} bytes", mft_record_size);

            println!(
                "Bytes per Cluster: {}, MFT Start LCN: {}",
                bytes_per_cluster, mft_start
            );

            // Desired number of MFT entries to parse
            let desired_entries = 5;
            let total_bytes_needed = desired_entries as u64 * mft_record_size;
            let bytes_per_cluster_u64 = bytes_per_cluster as u64;
            let clusters_needed =
                (total_bytes_needed + bytes_per_cluster_u64 - 1) / bytes_per_cluster_u64;

            println!(
                "Reading {} clusters to cover {} MFT entries ({} bytes)...",
                clusters_needed, desired_entries, total_bytes_needed
            );

            // Read the necessary number of clusters
            let mut mft_data = Vec::new();
            for cluster_index in 0..clusters_needed {
                let cluster = mft_start + cluster_index;
                println!("Reading cluster {}...", cluster);
                if let Some(mut cluster_data) =
                    read_raw_cluster(handle, cluster, bytes_per_cluster as usize)
                {
                    mft_data.append(&mut cluster_data);
                } else {
                    eprintln!("Failed to read cluster {}", cluster);
                    break;
                }
            }

            println!("Successfully read {} bytes of MFT data.", mft_data.len());

            // Parse and print the desired number of MFT entries
            println!("Parsing the first {} MFT entries:", desired_entries);
            parse_and_print_mft_entries(&mft_data, desired_entries, mft_record_size as usize);
        }

        // Close the handle when done
        unsafe {
            CloseHandle(handle);
        }

        // Wait for user input before exiting
        wait_for_enter();
    } else {
        println!("Program is not elevated. Relaunching as administrator...");

        match relaunch_as_admin() {
            Ok(module) if module.0 as usize > 32 => {
                println!("Successfully relaunched as administrator.");
                std::process::exit(0); // Exit the current process
            }
            Ok(module) => {
                eprintln!(
                    "Failed to relaunch as administrator. Error code: {}",
                    module.0
                );
            }
            Err(e) => {
                eprintln!("Failed to relaunch as administrator: {}", e);
            }
        }
    }
}

/// Waits for the user to press Enter.
fn wait_for_enter() {
    print!("Press Enter to exit...");
    io::stdout().flush().unwrap(); // Ensure the prompt is displayed immediately
    let _ = io::stdin().read_line(&mut String::new()); // Wait for user input
}
