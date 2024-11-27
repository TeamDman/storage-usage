use byte_unit::{Byte, Unit, UnitType};
use mft::MftParser;
use std::mem::size_of;
use tracing::{debug, info, warn};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Ioctl::{FSCTL_GET_NTFS_VOLUME_DATA, NTFS_VOLUME_DATA_BUFFER};
use windows::Win32::System::IO::DeviceIoControl;
use eyre::eyre;

use crate::win_handles::get_drive_handle;
use crate::win_paged_mft_reader::PagedMftReader;

/// Retrieves the NTFS volume data buffer.
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
    debug!("Read {} bytes of NTFS volume metadata", bytes_read);
    Ok(volume_data)
}

/// Reads and prints MFT data using PagedMftReader.
pub fn get_and_print_mft_data() -> eyre::Result<()> {
    // Step 1: Open the drive handle
    let drive_handle = get_drive_handle('C')?;
    let handle = *drive_handle; // Deref to get HANDLE

    // Step 2: Retrieve NTFS volume data
    let volume_data = get_mft_buffer(handle)?;
    debug!("Volume data: {:#?}", volume_data);

    let bytes_per_cluster = volume_data.BytesPerCluster as u64;

    // Step 3: Calculate MFT record size correctly
    // Apparently this can be negative but the mft lib is representing it as a u32
    // so we are casting here and ignoring some warnings about silly comparisons
    let mft_record_size = volume_data.BytesPerFileRecordSegment as i32;
    #[allow(unused_comparisons)]
    #[allow(clippy::absurd_extreme_comparisons)]
    let mft_record_size = if volume_data.BytesPerFileRecordSegment < 0 {
        warn!("Negative MFT record size: {}", mft_record_size);
        2u64.pow(-mft_record_size as u32) as u64
    } else {
        mft_record_size as u64
    };
    info!("MFT record size: {}", mft_record_size);

    let mft_start_offset = volume_data.MftStartLcn as u64 * bytes_per_cluster;
    let mft_valid_data_length = volume_data.MftValidDataLength as u64;

    debug!(
        "Bytes per cluster: {}, MFT start offset: {}, MFT valid data length: {}",
        bytes_per_cluster, mft_start_offset, mft_valid_data_length
    );

    // Step 4: Initialize PagedMftReader with desired buffer capacity (e.g., 10 MB)
    let buffer_capacity = Byte::from_u64_with_unit(10, Unit::MiB)
        .expect("Failed to create Byte instance")
        .as_u64() as usize;
    let mut paged_reader = PagedMftReader::new(handle, buffer_capacity, mft_start_offset, mft_valid_data_length);

    // Step 5: Initialize MftParser with PagedMftReader
    let mut parser = MftParser::from_read_seek(paged_reader, Some(mft_valid_data_length))?;

    // Step 6: Validate the first MFT entry
    let first_entry = parser.get_entry(0)?;
    if !first_entry.header.is_valid() {
        return Err(eyre!("First MFT entry has an invalid signature"));
    }

    // Step 7: Iterate over entries
    let mut invalid_count = 0;
    let mut total_logical = 0;
    let mut total_physical = 0;
    let mut total_entries = 0;

    for (i, entry) in parser.iter_entries().enumerate() {
        total_entries += 1;
        let should_log = i < 100 || i % 1000 == 0; // Log first 100 and every 1000th entry

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
                                "Found {} (physical={}, logical={})",
                                x.name,
                                Byte::from_u64(x.physical_size)
                                    .get_appropriate_unit(UnitType::Binary),
                                Byte::from_u64(x.logical_size)
                                    .get_appropriate_unit(UnitType::Binary)
                            );
                        }
                    }
                }
                None => {
                    // Entries without a name attribute can be ignored or handled as needed
                    // For debugging:
                    // warn!("Entry without name attribute: {:?}", e.header);
                }
            },
            Err(err) => {
                warn!("Error reading entry {}: {}", i + 1, err);
                break;
            }
        }
    }

    if invalid_count > 0 {
        warn!("Found {} invalid entries", invalid_count);
    }
    info!("Total entries: {}", total_entries);
    info!(
        "Total logical size: {}",
        Byte::from_u64(total_logical).get_appropriate_unit(UnitType::Binary)
    );
    info!(
        "Total physical size: {}",
        Byte::from_u64(total_physical).get_appropriate_unit(UnitType::Binary)
    );

    Ok(())
}
