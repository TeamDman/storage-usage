use crate::tui::progress::MftFileProgress;
use ratatui::text::Line;
use std::path::PathBuf;
use std::time::Instant;
use uom::si::f64::Information;

#[derive(Debug, Clone)]
pub enum MainboundMessage {
    FileSizeDiscovered {
        file_index: usize,
        file_size: Information,
    },
    EntrySizeDiscovered {
        file_index: usize,
        entry_size: Information,
    },
    Progress {
        file_index: usize,
        processed_size: Information,
    },
    DiscoveredFiles {
        file_index: usize,
        files: Vec<PathBuf>,
    },
    EntryStatus {
        file_index: usize,
        is_healthy: bool,
    },
    Complete {
        file_index: usize,
    },
    Error {
        file_index: usize,
        error: Line<'static>,
    },
}
impl MainboundMessage {
    pub fn handle(self, mft_files: &mut [MftFileProgress]) -> eyre::Result<()> {
        match self {
            MainboundMessage::FileSizeDiscovered {
                file_index,
                file_size,
            } => {
                mft_files[file_index].total_size = Some(file_size);
            }
            MainboundMessage::EntrySizeDiscovered {
                file_index,
                entry_size,
            } => {
                mft_files[file_index].entry_size = Some(entry_size);
            }
            MainboundMessage::Progress {
                file_index,
                processed_size,
            } => {
                let progress = &mut mft_files[file_index];
                progress.processed_size += processed_size;
            }
            MainboundMessage::Complete { file_index } => {
                mft_files[file_index].processing_end = Some(Instant::now());
            }
            MainboundMessage::Error { file_index, error } => {
                mft_files[file_index].errors.push(error);
            }
            MainboundMessage::DiscoveredFiles { file_index, files } => {
                let progress = &mut mft_files[file_index];
                progress.files_within.extend(files);
            }
            MainboundMessage::EntryStatus {
                file_index,
                is_healthy,
            } => {
                let progress = &mut mft_files[file_index];
                progress.entry_health_statuses.push(is_healthy);
            }
        }
        Ok(())
    }
}
