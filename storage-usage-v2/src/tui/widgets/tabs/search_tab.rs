use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use nucleo::Nucleo;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
struct FileEntry {
    path: PathBuf,
    display_name: String,
    full_path: String,
}

pub struct SearchTab {
    search_query: String,
    scroll_offset: usize,
    selected_index: usize,
    matcher: Nucleo<FileEntry>,
    last_file_count: usize,
    last_update: Instant,
    visible_height: usize,
}

impl Default for SearchTab {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchTab {
    pub fn new() -> Self {
        let config = nucleo::Config::DEFAULT;
        let matcher = Nucleo::new(
            config,
            Arc::new(|| {}), // notify callback - we'll handle updates in render
            None,            // use default number of threads
            1,               // single column for matching
        );

        Self {
            search_query: String::new(),
            scroll_offset: 0,
            selected_index: 0,
            matcher,
            last_file_count: 0,
            last_update: Instant::now(),
            visible_height: 20, // default, will be updated in render
        }
    }

    pub fn on_key(&mut self, event: KeyEvent) -> KeyboardResponse {
        match event.code {
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.scroll_offset = 0;
                self.selected_index = 0;
                self.update_search();
                KeyboardResponse::Consume
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.scroll_offset = 0;
                self.selected_index = 0;
                self.update_search();
                KeyboardResponse::Consume
            }
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    if self.selected_index < self.scroll_offset {
                        self.scroll_offset = self.selected_index;
                    }
                }
                KeyboardResponse::Consume
            }
            KeyCode::Down => {
                let snapshot = self.matcher.snapshot();
                let matched_count = snapshot.matched_item_count() as usize;
                if matched_count > 0 && self.selected_index < matched_count - 1 {
                    self.selected_index += 1;
                    if self.selected_index >= self.scroll_offset + self.visible_height {
                        self.scroll_offset =
                            self.selected_index.saturating_sub(self.visible_height - 1);
                    }
                }
                KeyboardResponse::Consume
            }
            KeyCode::PageUp => {
                self.selected_index = self.selected_index.saturating_sub(self.visible_height);
                self.scroll_offset = self.scroll_offset.saturating_sub(self.visible_height);
                KeyboardResponse::Consume
            }
            KeyCode::PageDown => {
                let snapshot = self.matcher.snapshot();
                let matched_count = snapshot.matched_item_count() as usize;
                if matched_count > 0 {
                    self.selected_index =
                        (self.selected_index + self.visible_height).min(matched_count - 1);
                    if self.selected_index >= self.scroll_offset + self.visible_height {
                        self.scroll_offset =
                            self.selected_index.saturating_sub(self.visible_height - 1);
                    }
                }
                KeyboardResponse::Consume
            }
            KeyCode::Home => {
                self.selected_index = 0;
                self.scroll_offset = 0;
                KeyboardResponse::Consume
            }
            KeyCode::End => {
                let snapshot = self.matcher.snapshot();
                let matched_count = snapshot.matched_item_count() as usize;
                if matched_count > 0 {
                    self.selected_index = matched_count - 1;
                    self.scroll_offset = matched_count.saturating_sub(self.visible_height);
                }
                KeyboardResponse::Consume
            }
            _ => KeyboardResponse::Pass,
        }
    }

    fn update_search(&mut self) {
        // Update the pattern for fuzzy matching
        self.matcher.pattern.reparse(
            0, // column 0
            &self.search_query,
            nucleo::pattern::CaseMatching::Smart,
            nucleo::pattern::Normalization::Smart,
            false, // assume new pattern for simplicity
        );
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, mft_files: &[MftFileProgress]) {
        let layout = Layout::vertical([
            Constraint::Length(1), // Search input (no border, just text)
            Constraint::Min(0),    // Results
        ]);
        let [search_area, results_area] = layout.areas(area);

        self.visible_height = results_area.height as usize;

        self.render_search_input(search_area, buf);
        self.update_file_entries(mft_files);
        self.render_search_results(results_area, buf);
    }

    fn render_search_input(&self, area: Rect, buf: &mut Buffer) {
        let search_text = format!(
            "Search: {} (Type to search, ↑↓ to navigate, PgUp/PgDn to scroll)",
            self.search_query
        );

        Paragraph::new(search_text)
            .style(Style::default().fg(Color::White))
            .render(area, buf);
    }

    fn update_file_entries(&mut self, mft_files: &[MftFileProgress]) {
        // Count total files to see if we need to update
        let total_files: usize = mft_files.iter().map(|mft| mft.files_within.len()).sum();

        // Only update if file count changed or it's been a while since last update
        let should_update =
            total_files != self.last_file_count || self.last_update.elapsed().as_millis() > 500; // Update every 500ms max

        if should_update && total_files > self.last_file_count {
            let injector = self.matcher.injector();

            // Add only new files since last update
            let mut files_added = 0;
            let mut current_count = 0;

            'outer: for file_progress in mft_files {
                for file_path in &file_progress.files_within {
                    current_count += 1;

                    // Skip files we've already added
                    if current_count <= self.last_file_count {
                        continue;
                    }

                    let display_name = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let full_path = file_path.to_string_lossy().to_string();

                    let entry = FileEntry {
                        path: file_path.clone(),
                        display_name: display_name.clone(),
                        full_path: full_path.clone(),
                    };

                    injector.push(entry, |entry, columns| {
                        // Use filename for primary matching, but include full path for context
                        columns[0] = format!("{} {}", entry.display_name, entry.full_path).into();
                    });

                    files_added += 1;

                    // Batch limit to avoid blocking UI too long
                    if files_added >= 10000 {
                        break 'outer;
                    }
                }
            }

            self.last_file_count = current_count;
            self.last_update = Instant::now();
        }

        // Tick the matcher to process any pending work
        self.matcher.tick(10); // 10ms timeout
    }

    fn render_search_results(&mut self, area: Rect, buf: &mut Buffer) {
        let snapshot = self.matcher.snapshot();
        let matched_count = snapshot.matched_item_count() as usize;

        if matched_count == 0 {
            let message = if self.search_query.is_empty() {
                "No files discovered yet. Files will appear here as MFT processing progresses."
            } else {
                "No files found matching search criteria."
            };

            Paragraph::new(message)
                .style(Style::default().fg(Color::Gray))
                .render(area, buf);
            return;
        }

        // Ensure scroll bounds are valid
        let max_scroll = matched_count.saturating_sub(self.visible_height);
        self.scroll_offset = self.scroll_offset.min(max_scroll);

        // Ensure selected index is valid
        self.selected_index = self.selected_index.min(matched_count - 1);

        // Get visible range
        let start = self.scroll_offset;
        let end = (start + self.visible_height).min(matched_count);

        let items: Vec<ListItem> = snapshot
            .matched_items(start as u32..end as u32)
            .enumerate()
            .map(|(idx, item)| {
                let global_idx = start + idx;
                let is_selected = global_idx == self.selected_index;

                let display_path = if let Some(parent) = item.data.path.parent() {
                    format!("{} ({})", item.data.display_name, parent.display())
                } else {
                    item.data.display_name.clone()
                };

                if !self.search_query.is_empty() {
                    // Highlight matches if we have a search query
                    // For now, just use different styling for selected items
                    let style = if is_selected {
                        Style::default().fg(Color::Black).bg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    ListItem::new(Line::from(Span::styled(display_path, style)))
                } else {
                    let style = if is_selected {
                        Style::default().fg(Color::Black).bg(Color::Yellow)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(Span::styled(display_path, style)))
                }
            })
            .collect();

        List::new(items).render(area, buf);
    }

    /// Clear all files from the matcher (useful when starting a new MFT scan)
    pub fn clear_files(&mut self) {
        // Create a new matcher to clear all data
        let config = nucleo::Config::DEFAULT;
        self.matcher = Nucleo::new(config, Arc::new(|| {}), None, 1);
        self.last_file_count = 0;
        self.scroll_offset = 0;
        self.selected_index = 0;
        self.last_update = Instant::now();
    }

    /// Get search statistics
    pub fn get_stats(&self) -> (usize, usize) {
        let snapshot = self.matcher.snapshot();
        (
            snapshot.item_count() as usize,
            snapshot.matched_item_count() as usize,
        )
    }

    /// Get the currently selected file path, if any
    pub fn get_selected_file(&self) -> Option<PathBuf> {
        let snapshot = self.matcher.snapshot();
        snapshot
            .get_matched_item(self.selected_index as u32)
            .map(|item| item.data.path.clone())
    }
}
