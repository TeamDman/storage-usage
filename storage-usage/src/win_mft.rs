use byte_unit::Byte;
use byte_unit::UnitType;
use eyre::bail;
use eyre::Context;
use std::io::Write;
use std::mem::size_of;
use std::process::Command;
use thousands::Separable;
use tracing::debug;
use tracing::info;
use tracing::warn;
use windows::core::GUID;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Ioctl::FSCTL_GET_NTFS_VOLUME_DATA;
use windows::Win32::System::Ioctl::NTFS_EXTENDED_VOLUME_DATA;
use windows::Win32::System::Ioctl::NTFS_VOLUME_DATA_BUFFER;
use windows::Win32::System::IO::DeviceIoControl;

pub fn get_ntfs_extended_volume_data(
    handle: HANDLE,
) -> eyre::Result<(NTFS_VOLUME_DATA_BUFFER, NTFS_EXTENDED_VOLUME_DATA, GUID)> {
    let size_data_buffer = size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32;
    let size_extended_data = size_of::<NTFS_EXTENDED_VOLUME_DATA>() as u32;
    let size_guid = size_of::<GUID>() as u32;
    let buffer_size = size_data_buffer + size_extended_data + size_guid;
    let mut buffer = vec![0u8; buffer_size as usize];
    let mut bytes_returned = 0u32;

    unsafe {
        DeviceIoControl(
            handle,
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
            0,
            Some(buffer.as_mut_ptr() as *mut _),
            buffer.len() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .ok()
        .context("Getting filesystem statistics")?;
    }

    debug!(
        "Bytes returned: {}, expected {} + {} + {}",
        bytes_returned, size_data_buffer, size_extended_data, size_guid
    );
    if bytes_returned < size_data_buffer {
        bail!(
            "NTFS volume data not available, got {} bytes but expected at least {}",
            bytes_returned,
            size_data_buffer
        );
    } else if bytes_returned < size_data_buffer + size_extended_data {
        warn!("Extended volume data not available");
    } else if bytes_returned < buffer_size as u32 {
        warn!("Resource manager identifier not available");
    }

    let volume_data = unsafe { std::ptr::read(buffer.as_ptr() as *const NTFS_VOLUME_DATA_BUFFER) };
    let extended_data = unsafe {
        std::ptr::read(buffer[size_of::<NTFS_VOLUME_DATA_BUFFER>()..].as_ptr()
            as *const NTFS_EXTENDED_VOLUME_DATA)
    };
    let resource_manager_identifier = unsafe {
        std::ptr::read(
            buffer[size_of::<NTFS_VOLUME_DATA_BUFFER>() + size_of::<NTFS_EXTENDED_VOLUME_DATA>()..]
                .as_ptr() as *const GUID,
        )
    };
    Ok((volume_data, extended_data, resource_manager_identifier))
}

/// Displays a summary of the MFT volume data.
pub fn display_mft_summary(
    volume_data: &NTFS_VOLUME_DATA_BUFFER,
    extended_data: &NTFS_EXTENDED_VOLUME_DATA,
    resource_manager_identifier: &GUID,
) -> eyre::Result<()> {
    // debug!("Volume data: {:#?}", volume_data);
    // ❯ fsutil fsinfo ntfsinfo C:

    info!("Windows printout (fsutil fsinfo ntfsinfo C:)");
    let theirs = Command::new("fsutil")
        .arg("fsinfo")
        .arg("ntfsinfo")
        .arg("C:")
        .output()
        .expect("tried to run fsutil fsinfo ntfsinfo C:")
        .stdout;
    std::io::stdout().write_all(&theirs).unwrap();

    info!("Our printout");

    //   NTFS Volume Serial Number :        0x123123123
    println!(
        "NTFS Volume Serial Number :        0x{:016x}",
        volume_data.VolumeSerialNumber
    );

    //   NTFS Version      :                3.1
    println!(
        "NTFS Version      :                {}.{}",
        extended_data.MajorVersion, extended_data.MinorVersion
    );

    //   LFS Version       :                2.0
    println!(
        "LFS Version       :                {}.{}",
        extended_data.LfsMajorVersion, extended_data.LfsMinorVersion
    );

    //   Total Sectors     :                1,953,488,895  (931.5 GB)
    println!(
        "Total Sectors     :                {} ({:#.1})",
        volume_data.NumberSectors.separate_with_commas(),
        Byte::from_u64(volume_data.NumberSectors as u64 * volume_data.BytesPerSector as u64)
            .get_appropriate_unit(UnitType::Binary)
    );

    //   Total Clusters    :                  244,186,111  (931.5 GB)
    println!(
        "Total Clusters    :                  {} ({:#.1})",
        volume_data.TotalClusters.separate_with_commas(),
        Byte::from_u64(volume_data.TotalClusters as u64 * volume_data.BytesPerCluster as u64)
            .get_appropriate_unit(UnitType::Binary)
    );

    //   Free Clusters     :                    7,058,154  ( 26.9 GB)
    println!(
        "Free Clusters     :                    {} ({:#.1})",
        volume_data.FreeClusters.separate_with_commas(),
        Byte::from_u64(volume_data.FreeClusters as u64 * volume_data.BytesPerCluster as u64)
            .get_appropriate_unit(UnitType::Binary)
    );

    //   Total Reserved Clusters :              1,827,471  (  7.0 GB)
    println!(
        "Total Reserved Clusters :              {} ({:#.1})",
        volume_data.TotalReserved.separate_with_commas(),
        Byte::from_u64(volume_data.TotalReserved as u64 * volume_data.BytesPerCluster as u64)
            .get_appropriate_unit(UnitType::Binary)
    );

    //   Reserved For Storage Reserve :         1,801,629  (  6.9 GB)
    println!(
        "Reserved For Storage Reserve :         {} ({:#.1})",
        volume_data.TotalReserved.separate_with_commas(),
        Byte::from_u64(volume_data.TotalReserved as u64 * volume_data.BytesPerCluster as u64)
            .get_appropriate_unit(UnitType::Binary)
    );

    //   Bytes Per Sector  :                512
    println!(
        "Bytes Per Sector  :                {}",
        volume_data.BytesPerSector
    );

    //   Bytes Per Physical Sector :        4096
    println!(
        "Bytes Per Physical Sector :        {}",
        extended_data.BytesPerPhysicalSector
    );

    //   Bytes Per Cluster :                4096
    println!(
        "Bytes Per Cluster :                {}",
        volume_data.BytesPerCluster
    );

    //   Bytes Per FileRecord Segment    :  1024
    println!(
        "Bytes Per FileRecord Segment    :  {}",
        volume_data.BytesPerFileRecordSegment
    );

    //   Clusters Per FileRecord Segment :  0
    println!(
        "Clusters Per FileRecord Segment :  {}",
        volume_data.ClustersPerFileRecordSegment
    );

    //   Mft Valid Data Length :            3.55 GB
    println!(
        "Mft Valid Data Length :            {:#.2}",
        Byte::from_u64(volume_data.MftValidDataLength as u64)
            .get_appropriate_unit(UnitType::Binary)
    );

    //   Mft Start Lcn  :                   0x00000000000c0000
    println!(
        "Mft Start Lcn  :                   0x{:016x}",
        volume_data.MftStartLcn
    );

    //   Mft2 Start Lcn :                   0x0000000000000002
    println!(
        "Mft2 Start Lcn :                   0x{:016x}",
        volume_data.Mft2StartLcn
    );

    //   Mft Zone Start :                   0x000000000a8821c0
    println!(
        "Mft Zone Start :                   0x{:016x}",
        volume_data.MftZoneStart
    );

    //   Mft Zone End   :                   0x000000000a885b00
    println!(
        "Mft Zone End   :                   0x{:016x}",
        volume_data.MftZoneEnd
    );

    //   MFT Zone Size  :                   57.25 MB
    println!(
        "MFT Zone Size  :                   {:#.2}",
        Byte::from_u64(
            (volume_data.MftZoneEnd - volume_data.MftZoneStart) as u64
                * volume_data.BytesPerCluster as u64
        )
        .get_appropriate_unit(UnitType::Binary)
    );

    //   Max Device Trim Extent Count :     256
    println!(
        "Max Device Trim Extent Count :     {}",
        extended_data.MaxDeviceTrimExtentCount
    );

    //   Max Device Trim Byte Count :       0xffffffff
    println!(
        "Max Device Trim Byte Count :       0x{:x}",
        extended_data.MaxDeviceTrimByteCount
    );

    //   Max Volume Trim Extent Count :     62
    println!(
        "Max Volume Trim Extent Count :     {}",
        extended_data.MaxVolumeTrimExtentCount
    );

    //   Max Volume Trim Byte Count :       0x40000000
    println!(
        "Max Volume Trim Byte Count :       0x{:x}",
        extended_data.MaxVolumeTrimByteCount
    );

    //   Resource Manager Identifier :      00000000-0000-0000-0000-000000000000
    println!(
        "Resource Manager Identifier :      {}",
        uuid_to_string(*resource_manager_identifier)
    );
    Ok(())
}

fn uuid_to_string(guid: GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        guid.data1,
        guid.data2,
        guid.data3,
        u16::from_be_bytes([guid.data4[0], guid.data4[1]]),
        u64::from_be_bytes([
            guid.data4[2],
            guid.data4[3],
            guid.data4[4],
            guid.data4[5],
            guid.data4[6],
            guid.data4[7],
            0,
            0
        ]) >> 16
    )
}
