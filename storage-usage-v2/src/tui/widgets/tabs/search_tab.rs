use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::path::PathBuf;

pub struct SearchTab {
    search_query: String,
    filtered_files: Vec<PathBuf>,
    scroll_offset: usize,
}

impl SearchTab {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            filtered_files: Vec::new(),
            scroll_offset: 0,
        }
    }

    pub fn on_key(&mut self, event: KeyEvent) -> KeyboardResponse {
        match event.code {
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.scroll_offset = 0; // Reset scroll when search changes
                KeyboardResponse::Consume
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.scroll_offset = 0;
                KeyboardResponse::Consume
            }
            KeyCode::Up => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
                KeyboardResponse::Consume
            }
            KeyCode::Down => {
                self.scroll_offset += 1; // Will be clamped in render
                KeyboardResponse::Consume
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                KeyboardResponse::Consume
            }
            KeyCode::PageDown => {
                self.scroll_offset += 10; // Will be clamped in render
                KeyboardResponse::Consume
            }
            _ => KeyboardResponse::Pass,
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, mft_files: &[MftFileProgress]) {
        let layout = Layout::vertical([
            Constraint::Length(3), // Search input
            Constraint::Min(0),    // Results
        ]);
        let [search_area, results_area] = layout.areas(area);

        self.render_search_input(search_area, buf);
        self.update_filtered_files(mft_files);
        self.render_search_results(results_area, buf);
    }

    fn render_search_input(&self, area: Rect, buf: &mut Buffer) {
        let search_text = format!(
            "Search: {} (Type to search, ↑↓ to scroll)",
            self.search_query
        );

        Paragraph::new(search_text)
            .block(Block::default().borders(Borders::ALL).title("File Search"))
            .render(area, buf);
    }

    fn update_filtered_files(&mut self, mft_files: &[MftFileProgress]) {
        self.filtered_files.clear();

        if self.search_query.is_empty() {
            // Show all files when no search query
            for file_progress in mft_files {
                self.filtered_files
                    .extend(file_progress.files_within.iter().cloned());
            }
        } else {
            // Filter files based on search query
            let query_lower = self.search_query.to_lowercase();
            for file_progress in mft_files {
                for file_path in &file_progress.files_within {
                    if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                        if file_name.to_lowercase().contains(&query_lower) {
                            self.filtered_files.push(file_path.clone());
                        }
                    }

                    // Also search in full path
                    if file_path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query_lower)
                    {
                        if !self.filtered_files.contains(file_path) {
                            self.filtered_files.push(file_path.clone());
                        }
                    }
                }
            }
        }
    }

    fn render_search_results(&mut self, area: Rect, buf: &mut Buffer) {
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders

        // Clamp scroll offset
        let max_scroll = self.filtered_files.len().saturating_sub(visible_height);
        self.scroll_offset = self.scroll_offset.min(max_scroll);

        let visible_files = if self.filtered_files.is_empty() {
            vec![]
        } else {
            let start = self.scroll_offset;
            let end = (start + visible_height).min(self.filtered_files.len());
            self.filtered_files[start..end].to_vec()
        };

        if visible_files.is_empty() {
            let message = if self.search_query.is_empty() {
                "No files discovered yet. Files will appear here as MFT processing progresses."
            } else {
                "No files found matching search criteria."
            };

            Paragraph::new(message)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("Results (0 files)")),
                )
                .render(area, buf);
        } else {
            let items: Vec<ListItem> = visible_files
                .iter()
                .map(|path| {
                    let display_path =
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            if let Some(parent) = path.parent() {
                                format!("{} ({})", file_name, parent.display())
                            } else {
                                file_name.to_string()
                            }
                        } else {
                            path.display().to_string()
                        };

                    ListItem::new(display_path)
                })
                .collect();

            let title = if self.search_query.is_empty() {
                format!("All Files ({} total)", self.filtered_files.len())
            } else {
                format!("Search Results ({} matches)", self.filtered_files.len())
            };

            List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .render(area, buf);
        }
    }
}
