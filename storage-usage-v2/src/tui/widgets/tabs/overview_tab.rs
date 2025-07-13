use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::time::Instant;
use uom::si::information::byte;

pub struct OverviewTab;

impl OverviewTab {
    pub fn new() -> Self {
        Self
    }

    pub fn on_key(&mut self, _event: KeyEvent) -> KeyboardResponse {
        KeyboardResponse::Pass
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        mft_files: &[MftFileProgress],
        processing_begin: Instant,
    ) {
        let layout = Layout::horizontal([
            Constraint::Percentage(50), // Summary stats
            Constraint::Percentage(50), // File list with progress
        ]);
        let [summary_area, files_area] = layout.areas(area);

        self.render_summary(summary_area, buf, mft_files, processing_begin);
        self.render_file_list(files_area, buf, mft_files);
    }

    fn render_summary(
        &self,
        area: Rect,
        buf: &mut Buffer,
        mft_files: &[MftFileProgress],
        processing_begin: Instant,
    ) {
        let completed_files = mft_files
            .iter()
            .filter(|f| f.processing_end.is_some())
            .count();
        let total_files = mft_files.len();
        let total_errors: usize = mft_files.iter().map(|f| f.errors.len()).sum();
        let elapsed = processing_begin.elapsed();

        let summary_text = format!(
            "Files: {}/{} | Errors: {} | Elapsed: {:.1}s",
            completed_files,
            total_files,
            total_errors,
            elapsed.as_secs_f64()
        );

        Paragraph::new(summary_text)
            .block(Block::default().borders(Borders::ALL).title("Summary"))
            .render(area, buf);
    }

    fn render_file_list(&self, area: Rect, buf: &mut Buffer, mft_files: &[MftFileProgress]) {
        let items: Vec<ListItem> = mft_files
            .iter()
            .map(|file| {
                let filename = file
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown");

                let mut text = Text::default();

                if file.processing_end.is_some() {
                    text.push_span(Span::raw("OK "));
                } else {
                    text.push_span(Span::raw("..."));
                }

                text.push_span(Span::raw(" "));

                text.push_span(Span::raw(filename));

                if let Some(total) = file.total_size {
                    let size_str = Self::format_size(total.get::<byte>());
                    text.push_span(Span::raw(format!(" - {}", size_str)));
                } else {
                    text.push_span(Span::raw(" - ?"));
                }

                if !file.errors.is_empty() {
                    text.push_span(Span::raw(" (Errors: "));
                    text.push_span(Span::raw(file.errors.len().to_string()));
                    text.push_span(Span::raw(")"));
                }

                ListItem::new(text)
            })
            .collect();

        List::new(items)
            .block(Block::default().borders(Borders::ALL).title("MFT Files"))
            .render(area, buf);
    }

    fn format_size(bytes: u64) -> String {
        if bytes >= 1_000_000_000 {
            format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
        } else if bytes >= 1_000_000 {
            format!("{:.1} MB", bytes as f64 / 1_000_000.0)
        } else if bytes >= 1_000 {
            format!("{:.1} KB", bytes as f64 / 1_000.0)
        } else {
            format!("{} B", bytes)
        }
    }
}
