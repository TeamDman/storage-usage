use mft::EntryHeader;
use mft::MftEntry;
use std::io::Cursor;
use uom::si::information::byte;
use uom::si::u64::Information;

pub struct MftEntryIterator<'a> {
    mft_bytes: &'a [u8],
    entry_size: u32,
    current_entry: u64,
    total_entries: u64,
}

impl<'a> MftEntryIterator<'a> {
    pub fn new(mft_bytes: &'a [u8]) -> Self {
        // Read the first entry to determine the entry size (same as MftParser does)
        let mut cursor = Cursor::new(mft_bytes);
        let entry_size = match EntryHeader::from_reader(&mut cursor, 0) {
            Ok(header) => header.total_entry_size,
            Err(_) => 1024, // Default MFT entry size if we can't read the first entry
        };
        
        let total_entries = mft_bytes.len() as u64 / entry_size as u64;
        
        Self {
            mft_bytes,
            entry_size,
            current_entry: 0,
            total_entries,
        }
    }
}

impl Iterator for MftEntryIterator<'_> {
    type Item = (Information, mft::err::Result<MftEntry>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_entry >= self.total_entries {
            return None;
        }
        
        let start_offset = (self.current_entry * self.entry_size as u64) as usize;
        let end_offset = start_offset + self.entry_size as usize;
        
        // Check bounds
        if end_offset > self.mft_bytes.len() {
            return None;
        }
        
        // Extract the entry buffer
        let entry_buffer = self.mft_bytes[start_offset..end_offset].to_vec();
        let size = Information::new::<byte>(self.entry_size as u64);
        
        let entry_number = self.current_entry;
        self.current_entry += 1;
        
        // Use MftEntry::from_buffer which properly handles fixups and validation
        let entry_result = MftEntry::from_buffer(entry_buffer, entry_number);
        
        Some((size, entry_result))
    }
}
