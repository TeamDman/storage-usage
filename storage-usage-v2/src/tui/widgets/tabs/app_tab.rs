use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use crate::tui::widgets::tabs::overview_tab::OverviewTab;
use crate::tui::widgets::tabs::search_tab::SearchTab;
use crate::tui::widgets::tabs::visualizer_tab::VisualizerTab;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use std::time::Instant;

pub enum AppTab {
    Overview(OverviewTab),
    Visualizer(VisualizerTab),
    Search(SearchTab),
}

impl AppTab {
    pub fn title(&self) -> &'static str {
        match self {
            AppTab::Overview(_) => "Overview",
            AppTab::Visualizer(_) => "Visualizer",
            AppTab::Search(_) => "Search",
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
            AppTab::Overview(tab) => tab.render(area, buf, mft_files, processing_begin),
            AppTab::Visualizer(tab) => tab.render(area, buf, mft_files),
            AppTab::Search(tab) => tab.render(area, buf, mft_files),
        }
    }

    pub fn on_key(&mut self, event: KeyEvent) -> KeyboardResponse {
        match self {
            AppTab::Overview(tab) => tab.on_key(event),
            AppTab::Visualizer(tab) => tab.on_key(event),
            AppTab::Search(tab) => tab.on_key(event),
        }
    }
}
