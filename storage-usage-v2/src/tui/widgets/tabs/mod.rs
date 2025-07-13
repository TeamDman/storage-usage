pub mod keyboard_response;
pub mod overview_tab;pub mod search_tab;
pub mod visualizer_tab;

use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use crate::tui::widgets::tabs::overview_tab::OverviewTab;
use crate::tui::widgets::tabs::search_tab::SearchTab;
use crate::tui::widgets::tabs::visualizer_tab::VisualizerTab;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::widgets::Tabs;
use std::time::Instant;

pub enum Tab {
    Overview(OverviewTab),
    Visualizer(VisualizerTab),
    Search(SearchTab),
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Overview(_) => "Overview",
            Tab::Visualizer(_) => "Visualizer",
            Tab::Search(_) => "Search",
        }
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        mft_files: &[MftFileProgress],
        processing_begin: Instant,
    ) {
        match self {
            Tab::Overview(tab) => tab.render(area, buf, mft_files, processing_begin),
            Tab::Visualizer(tab) => tab.render(area, buf, mft_files),
            Tab::Search(tab) => tab.render(area, buf, mft_files),
        }
    }

    pub fn on_key(&mut self, event: KeyEvent) -> KeyboardResponse {
        match self {
            Tab::Overview(tab) => tab.on_key(event),
            Tab::Visualizer(tab) => tab.on_key(event),
            Tab::Search(tab) => tab.on_key(event),
        }
    }
}

pub struct AppTabs {
    pub tabs: Vec<Tab>,
    pub selected: usize,
}
impl AppTabs {
    pub fn new() -> Self {
        Self {
            tabs: vec![
                Tab::Overview(OverviewTab::new()),
                Tab::Visualizer(VisualizerTab::new()),
                Tab::Search(SearchTab::new()),
            ],
            selected: 0,
        }
    }

    pub fn on_key(&mut self, event: KeyEvent) -> KeyboardResponse {
        match event.code {
            KeyCode::Left => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                KeyboardResponse::Consume
            }
            KeyCode::Right => {
                if self.selected < self.tabs.len() - 1 {
                    self.selected += 1;
                }
                KeyboardResponse::Consume
            }
            _ => self.tabs[self.selected].on_key(event),
        }
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        mft_files: &[MftFileProgress],
        processing_begin: Instant,
    ) {
        let vertical_layout = Layout::vertical([
            Constraint::Length(3), // tabs header with border
            Constraint::Min(0),    // body
        ]);
        let [tabs_area, body_area] = vertical_layout.areas(area);

        // Render tab titles
        let titles: Vec<&str> = self.tabs.iter().map(|t| t.title()).collect();
        let tabs_widget = Tabs::new(titles)
            .block(Block::bordered().title("MFT Analysis"))
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
.highlight_style(Style::default().fg(Color::Yellow).add_modifier(ratatui::style::Modifier::BOLD))
            .select(self.selected);
        use ratatui::widgets::Widget;
        tabs_widget.render(tabs_area, buf);

        // Render selected tab content
        self.tabs[self.selected].render(body_area, buf, mft_files, processing_begin);
    }
}
