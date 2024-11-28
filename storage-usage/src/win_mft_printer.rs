use crate::win_handles::get_drive_handle;
use crate::win_mft::display_mft_summary;
use crate::win_mft::get_ntfs_extended_volume_data;
use crate::win_paged_mft_reader::PagedMftReader;
use byte_unit::Byte;
use byte_unit::Unit;
use byte_unit::UnitType;
use eyre::eyre;
use mft::attribute::data_run::RunType;
use mft::attribute::MftAttributeType;
use mft::MftParser;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Reads and prints MFT data using PagedMftReader.
pub fn get_and_print_mft_data() -> eyre::Result<()> {
    // Step 1: Open the drive handle
    let drive_handle = get_drive_handle('C')?;
    // Deref to get HANDLE

    // Step 2: Retrieve NTFS volume data
    let (volume_data, extended_data, resource_manager_identifier) =
        get_ntfs_extended_volume_data(*drive_handle)?;
    display_mft_summary(&volume_data, &extended_data, &resource_manager_identifier)?;

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
    let buffer_capacity = Byte::from_u64_with_unit(100, Unit::MiB)
        .expect("Failed to create Byte instance")
        .as_u64() as usize;
    let mut paged_reader = PagedMftReader::new(
        *drive_handle,
        buffer_capacity,
        mft_start_offset,
        mft_valid_data_length,
    );

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
    let mut total_actual = 0;
    let mut total_entries = 0;
    for (i, entry) in parser.iter_entries().enumerate() {
        total_entries += 1;
        let should_log = i < 100 || i % 5000 == 0; // Log first 100 and every 1000th entry

        match entry {
            Ok(e) if !e.header.is_valid() => {
                invalid_count += 1;
            }
            Ok(e) => {
                // Sum up sizes from $DATA attributes
                let mut entry_logical_size = 0;
                let mut entry_physical_size = 0;

                for attr_result in e.iter_attributes_matching(Some(vec![MftAttributeType::DATA])) {
                    match attr_result {
                        Ok(attr) if attr.header.type_code == MftAttributeType::DATA => {
                            match attr.data {
                                mft::attribute::MftAttributeContent::Raw(raw_attribute) => {
                                    entry_logical_size += raw_attribute.data.len() as u64;
                                    entry_physical_size += raw_attribute.data.len() as u64;
                                }
                                mft::attribute::MftAttributeContent::AttrX10(
                                    standard_info_attr,
                                ) => {}
                                mft::attribute::MftAttributeContent::AttrX20(
                                    attribute_list_attr,
                                ) => {}
                                mft::attribute::MftAttributeContent::AttrX30(file_name_attr) => {
                                    entry_logical_size += file_name_attr.logical_size;
                                    entry_physical_size += file_name_attr.physical_size;
                                }
                                mft::attribute::MftAttributeContent::AttrX40(object_id_attr) => {}
                                mft::attribute::MftAttributeContent::AttrX80(data_attr) => {
                                    entry_logical_size += data_attr.data().len() as u64;
                                    entry_physical_size += data_attr.data().len() as u64;
                                }
                                mft::attribute::MftAttributeContent::AttrX90(index_root_attr) => {}
                                mft::attribute::MftAttributeContent::DataRun(non_resident_attr) => {
                                    for run in non_resident_attr.data_runs.iter() {
                                        entry_logical_size += run.lcn_length * bytes_per_cluster;
                                        if run.run_type == RunType::Standard {
                                            entry_physical_size +=
                                                run.lcn_length * bytes_per_cluster;
                                        }
                                    }
                                }
                                mft::attribute::MftAttributeContent::None => {}
                            }
                        }
                        _ => {}
                    }
                }

                total_logical += entry_logical_size;
                total_physical += entry_physical_size;
                total_actual += mft_record_size;

                // Get the file name for logging
                let name = e
                    .find_best_name_attribute()
                    .map(|x| x.name.clone())
                    .unwrap_or_else(|| "<unknown>".to_string());

                if e.is_dir() {
                    if should_log {
                        info!("Found dir {}", name);
                    }
                } else {
                    if should_log {
                        info!(
                            "Found {} (physical={:#.2}, logical={:#.2})",
                            name,
                            Byte::from_u64(entry_physical_size)
                                .get_appropriate_unit(UnitType::Binary),
                            Byte::from_u64(entry_logical_size)
                                .get_appropriate_unit(UnitType::Binary)
                        );
                    }
                }
            }
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
        "Total logical size: {:#.2}",
        Byte::from_u64(total_logical).get_appropriate_unit(UnitType::Binary)
    );
    info!(
        "Total physical size: {:#.2}",
        Byte::from_u64(total_physical).get_appropriate_unit(UnitType::Binary)
    );
    info!(
        "Total actual size: {:#.2}",
        Byte::from_u64(total_actual).get_appropriate_unit(UnitType::Binary)
    );

    Ok(())
}
