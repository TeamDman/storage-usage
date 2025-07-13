use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Gauge;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

pub struct VisualizerTab {
    selected_file: usize,
}

impl VisualizerTab {
    pub fn new() -> Self {
        Self { selected_file: 0 }
    }

    pub fn on_key(&mut self, event: KeyEvent) -> KeyboardResponse {
        use ratatui::crossterm::event::KeyCode;

        match event.code {
            KeyCode::Up => {
                if self.selected_file > 0 {
                    self.selected_file -= 1;
                }
                KeyboardResponse::Consume
            }
            KeyCode::Down => {
                self.selected_file += 1; // Will be clamped in render
                KeyboardResponse::Consume
            }
            _ => KeyboardResponse::Pass,
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, mft_files: &[MftFileProgress]) {
        if mft_files.is_empty() {
            Paragraph::new("No MFT files loaded")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Entry Health Visualizer"),
                )
                .render(area, buf);
            return;
        }

        // Clamp selected_file to valid range
        self.selected_file = self.selected_file.min(mft_files.len() - 1);

        let layout = Layout::vertical([
            Constraint::Length(3), // File selector
            Constraint::Min(0),    // Visualization area
        ]);
        let [selector_area, viz_area] = layout.areas(area);

        self.render_file_selector(selector_area, buf, mft_files);
        self.render_entry_health_visualization(viz_area, buf, &mft_files[self.selected_file]);
    }

    fn render_file_selector(&self, area: Rect, buf: &mut Buffer, mft_files: &[MftFileProgress]) {
        let filename = mft_files[self.selected_file]
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        let text = format!(
            "File {}/{}: {} (Use ↑↓ to navigate)",
            self.selected_file + 1,
            mft_files.len(),
            filename
        );

        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Entry Health Visualizer"),
            )
            .render(area, buf);
    }

    fn render_entry_health_visualization(
        &self,
        area: Rect,
        buf: &mut Buffer,
        file: &MftFileProgress,
    ) {
        if file.entry_health_statuses.is_empty() {
            Paragraph::new("No entry health data available yet")
                .style(Style::default().fg(Color::Gray))
                .render(area, buf);
            return;
        }

        let healthy_count = file.entry_health_statuses.iter().filter(|&&h| h).count();
        let total_count = file.entry_health_statuses.len();
        let health_ratio = if total_count > 0 {
            healthy_count as f64 / total_count as f64
        } else {
            0.0
        };

        let layout = Layout::vertical([
            Constraint::Length(3), // Health statistics
            Constraint::Min(0),    // Visual representation
        ]);
        let [stats_area, visual_area] = layout.areas(area);

        // Render health statistics
        let stats_text = format!(
            "Healthy entries: {}/{} ({:.1}%)",
            healthy_count,
            total_count,
            health_ratio * 100.0
        );

        Gauge::default()
            .gauge_style(Style::default().fg(if health_ratio > 0.9 {
                Color::Green
            } else if health_ratio > 0.7 {
                Color::Yellow
            } else {
                Color::Red
            }))
            .ratio(health_ratio)
            .label(stats_text)
            .render(stats_area, buf);

        // Render visual grid of entry health
        self.render_health_grid(visual_area, buf, &file.entry_health_statuses);
    }

    fn render_health_grid(&self, area: Rect, buf: &mut Buffer, health_statuses: &[bool]) {
        let grid_width = area.width as usize;
        let grid_height = area.height as usize;
        let total_cells = grid_width * grid_height;

        if total_cells == 0 {
            return;
        }

        let entries_per_cell = (health_statuses.len() + total_cells - 1) / total_cells;

        for y in 0..grid_height {
            for x in 0..grid_width {
                let cell_index = y * grid_width + x;
                let start_entry = cell_index * entries_per_cell;
                let end_entry = (start_entry + entries_per_cell).min(health_statuses.len());

                if start_entry >= health_statuses.len() {
                    break;
                }

                let cell_health = if start_entry < end_entry {
                    let healthy_in_cell = health_statuses[start_entry..end_entry]
                        .iter()
                        .filter(|&&h| h)
                        .count();
                    let total_in_cell = end_entry - start_entry;
                    healthy_in_cell as f64 / total_in_cell as f64
                } else {
                    1.0
                };

                let color = if cell_health > 0.9 {
                    Color::Green
                } else if cell_health > 0.7 {
                    Color::Yellow
                } else if cell_health > 0.3 {
                    Color::Red
                } else {
                    Color::DarkGray
                };

                let symbol = if cell_health > 0.9 {
                    "█"
                } else if cell_health > 0.7 {
                    "▓"
                } else if cell_health > 0.3 {
                    "▒"
                } else {
                    "░"
                };

                if let Some(cell) = buf.cell_mut((area.x + x as u16, area.y + y as u16)) {
                    cell.set_symbol(symbol);
                    cell.set_fg(color);
                }
            }
        }
    }
}
