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
use windows::Win32::System::Ioctl::FSCTL_GET_RETRIEVAL_POINTERS;
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

impl Default for MftEntry {
    fn default() -> Self {
        MftEntry {
            signature: [0; 4],
            fixup_offset: 0,
            fixup_size: 0,
            log_file_sequence_number: 0,
            sequence_number: 0,
            hard_link_count: 0,
            first_attribute_offset: 0,
            flags: 0,
            used_size: 0,
            allocated_size: 0,
            file_reference_to_base: 0,
            next_attribute_id: 0,
            padding: [0; 2],
        }
    }
}

/// Structure for input to FSCTL_GET_RETRIEVAL_POINTERS
#[repr(C)]
struct STARTING_VCN_INPUT_BUFFER {
    StartingVcn: u64,
}

/// Structure representing a Retrieval Pointer
#[repr(C)]
struct RetrievalPointer {
    StartingLcn: u64,
    ClusterCount: u64,
}

/// Structure representing the Retrieval Pointers Buffer
#[repr(C)]
struct RETRIEVAL_POINTERS_BUFFER_FULL {
    StartingVcn: u64,
    ExtentCount: u32,
    _padding: u32,
    Extents: [RetrievalPointer; 1], // Placeholder for dynamic array
}

impl RETRIEVAL_POINTERS_BUFFER_FULL {
    /// Returns an iterator over the extents.
    fn extents(&self) -> &[RetrievalPointer] {
        unsafe {
            let ptr = self.Extents.as_ptr();
            let count = self.ExtentCount as usize;
            std::slice::from_raw_parts(ptr, count)
        }
    }
}

/// Retrieves NTFS volume data.
fn get_ntfs_volume_data(handle: HANDLE) -> Option<NTFS_VOLUME_DATA_BUFFER> {
    let mut ntfs_volume_data = NTFS_VOLUME_DATA_BUFFER::default();
    let mut bytes_returned = 0u32;

    let result = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
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

/// Retrieves the physical cluster numbers for the MFT.
fn get_mft_physical_location(handle: HANDLE, mft_start: u64) -> Option<Vec<u64>> {
    // Define input buffer for FSCTL_GET_RETRIEVAL_POINTERS
    let input = STARTING_VCN_INPUT_BUFFER {
        StartingVcn: mft_start,
    };

    // Define output buffer. For simplicity, assume up to 5 extents.
    let mut retrieval_buffer =
        vec![0u8; size_of::<RETRIEVAL_POINTERS_BUFFER_FULL>() + size_of::<RetrievalPointer>() * 5];
    let mut bytes_returned = 0u32;

    let result = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_GET_RETRIEVAL_POINTERS,
            Some(&input as *const _ as *const _),
            size_of::<STARTING_VCN_INPUT_BUFFER>() as u32,
            Some(retrieval_buffer.as_mut_ptr() as *mut _),
            retrieval_buffer.len() as u32,
            Some(&mut bytes_returned),
            None,
        )
    };

    if !result.as_bool() {
        eprintln!(
            "Failed to retrieve MFT physical location. Error: {:?}",
            unsafe { GetLastError() }
        );
        return None;
    }

    // Now, parse the retrieval buffer
    let retrieval_buffer_full_ptr =
        retrieval_buffer.as_ptr() as *const RETRIEVAL_POINTERS_BUFFER_FULL;
    let retrieval_buffer_full_struct = unsafe { &*retrieval_buffer_full_ptr };

    let clusters: Vec<u64> = retrieval_buffer_full_struct
        .extents()
        .iter()
        .map(|e| e.StartingLcn)
        .collect();

    Some(clusters)
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
        Some(buffer)
    } else {
        eprintln!("Failed to read raw cluster data. Error: {:?}", unsafe {
            GetLastError()
        });
        None
    }
}

/// Parses MFT entries and prints their disk usage.
fn parse_and_print_mft_entries(data: &[u8], count: usize) {
    for i in 0..count {
        let offset = i * 1024; // Assuming 1 KB per MFT entry
        if offset + 1024 > data.len() {
            eprintln!("Insufficient data for MFT entry {}", i + 1);
            break;
        }

        let entry_data = &data[offset..offset + 1024];
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
                None,
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

            println!(
                "Bytes per Cluster: {}, MFT Start LCN: {}",
                bytes_per_cluster, mft_start
            );

            // Retrieve MFT physical location
            if let Some(clusters) = get_mft_physical_location(handle, mft_start) {
                println!("MFT located at clusters: {:?}", clusters);

                // For simplicity, read the first cluster containing the MFT
                if let Some(first_cluster) = clusters.first() {
                    println!("Reading MFT from cluster: {}", first_cluster);

                    if let Some(mft_data) =
                        read_raw_cluster(handle, *first_cluster, bytes_per_cluster as usize)
                    {
                        println!("Successfully read MFT cluster.");

                        // Parse and print the first 5 MFT entries
                        println!("Parsing the first 5 MFT entries:");
                        parse_and_print_mft_entries(&mft_data, 5);
                    }
                }
            }
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
