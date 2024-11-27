use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use tracing::debug;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::Storage::FileSystem::SetFilePointerEx;
use windows::Win32::Storage::FileSystem::FILE_BEGIN;

/// A reader that paginates access to the MFT by reading in chunks.
/// It maps virtual positions (0..mft_valid_data_length) to physical disk positions (mft_start_offset..mft_start_offset + mft_valid_data_length).
pub struct PagedMftReader {
    handle: HANDLE,
    buffer: Vec<u8>,
    buffer_start: u64, // Virtual byte offset where the buffer starts in the MFT
    buffer_end: u64,   // Virtual byte offset where the buffer ends in the MFT
    current_pos: u64,  // Current virtual read position in the MFT
    buffer_capacity: usize, // Size of each buffer chunk
    base_offset: u64,  // Physical disk byte offset where the MFT starts
    total_size: u64,   // Total MFT size
}

impl PagedMftReader {
    /// Creates a new `PagedMftReader`.
    pub fn new(handle: HANDLE, buffer_capacity: usize, base_offset: u64, total_size: u64) -> Self {
        Self {
            handle,
            buffer: Vec::with_capacity(buffer_capacity),
            buffer_start: 0,
            buffer_end: 0,
            current_pos: 0,
            buffer_capacity,
            base_offset,
            total_size,
        }
    }

    /// Fills the buffer starting from `current_pos`.
    fn fill_buffer(&mut self) -> std::io::Result<()> {
        // Calculate physical disk offset
        let physical_offset = self.base_offset + self.current_pos;

        // Set file pointer to physical_offset
        unsafe {
            SetFilePointerEx(self.handle, physical_offset as i64, None, FILE_BEGIN).ok()?;
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
            "Filled buffer: virtual_start={} virtual_end={} bytes_read={}",
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
        // Calculate the new virtual position based on SeekFrom
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                let pos = self
                    .total_size
                    .checked_add(offset as i64 as u64)
                    .ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Seek overflow")
                    })?;
                pos
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

        // Ensure new_pos is within total_size
        if new_pos > self.total_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek position out of bounds",
            ));
        }

        // Update current_pos
        self.current_pos = new_pos;

        Ok(self.current_pos)
    }
}
