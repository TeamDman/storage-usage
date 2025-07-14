use ratatui::text::Line;
use std::path::PathBuf;
use std::time::Instant;
use uom::si::f64::Information;

pub struct MftFileProgress {
    pub path: PathBuf,
    pub total_size: Option<Information>,
    pub entry_size: Option<Information>,
    pub processed_size: Information,
    pub processing_end: Option<Instant>,
    pub files_within: Vec<PathBuf>,
    pub entry_health_statuses: Vec<bool>,
    pub errors: Vec<Line<'static>>,
}
