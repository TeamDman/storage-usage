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

/// A reader that paginates access to the MFT by reading in chunks.
pub struct PagedMftReader {
    handle: HANDLE,
    buffer: Vec<u8>,
    buffer_start: u64,      // Byte offset where the buffer starts in the MFT
    buffer_end: u64,        // Byte offset where the buffer ends in the MFT
    current_pos: u64,       // Current read position in the MFT
    buffer_capacity: usize, // Size of each buffer chunk
}

impl PagedMftReader {
    /// Creates a new `PagedMftReader`.
    pub fn new(handle: HANDLE, buffer_capacity: usize, starting_pos: u64) -> Self {
        Self {
            handle,
            buffer: Vec::with_capacity(buffer_capacity),
            buffer_start: 0,
            buffer_end: 0,
            current_pos: starting_pos,
            buffer_capacity,
        }
    }

    /// Fills the buffer starting from `current_pos`.
    fn fill_buffer(&mut self) -> std::io::Result<()> {
        // Set file pointer to current_pos
        unsafe {
            SetFilePointerEx(self.handle, self.current_pos as i64, None, FILE_BEGIN).ok()?;
        }

        // Clear the buffer and reserve space
        self.buffer.clear();
        self.buffer.resize(self.buffer_capacity, 0);

        let mut bytes_read = 0u32;
        // Read data into the buffer
        unsafe {
            ReadFile(
                self.handle,
                Some(self.buffer.as_mut_ptr() as *mut _),
                self.buffer_capacity as u32,
                Some(&mut bytes_read),
                None,
            )
            .ok()?;
        }

        // Truncate the buffer to the actual bytes read
        self.buffer.truncate(bytes_read as usize);
        self.buffer_start = self.current_pos;
        self.buffer_end = self.current_pos + bytes_read as u64;

        debug!(
            "Filled buffer: start={} end={} bytes_read={}",
            self.buffer_start, self.buffer_end, bytes_read
        );

        Ok(())
    }
}

impl Read for PagedMftReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // If the buffer is empty or current_pos is outside the buffer, refill it
        if self.current_pos < self.buffer_start || self.current_pos >= self.buffer_end {
            self.fill_buffer()?;
            // If no data was read, return EOF
            if self.buffer.is_empty() {
                return Ok(0);
            }
        }

        // Calculate the start index within the buffer
        let buffer_offset = (self.current_pos - self.buffer_start) as usize;
        let available = self.buffer.len().saturating_sub(buffer_offset);
        if available == 0 {
            return Ok(0); // EOF
        }

        // Determine how much to read
        let to_read = std::cmp::min(buf.len(), available);
        buf[..to_read].copy_from_slice(&self.buffer[buffer_offset..buffer_offset + to_read]);

        // Update the current position
        self.current_pos += to_read as u64;

        Ok(to_read)
    }
}

impl Seek for PagedMftReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        // Calculate the new position based on SeekFrom
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                // To implement SeekFrom::End, you'd need to know the total size.
                // For simplicity, let's return an error.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "SeekFrom::End is not supported",
                ));
            }
            SeekFrom::Current(offset) => {
                if offset >= 0 {
                    self.current_pos.checked_add(offset as u64).ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek position overflow",
                        )
                    })?
                } else {
                    self.current_pos
                        .checked_sub((-offset) as u64)
                        .ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Seek position underflow",
                            )
                        })?
                }
            }
        };

        // Update current_pos
        self.current_pos = new_pos;

        Ok(self.current_pos)
    }
}
