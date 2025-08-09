use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::app_tab::AppTab;
use crate::tui::widgets::tabs::errors_tab::ErrorsTab;
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
use ratatui::style::Stylize;
use ratatui::symbols::border::PROPORTIONAL_TALL;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Padding;
use ratatui::widgets::Tabs;
use ratatui::widgets::Widget;
use std::time::Instant;

pub struct AppTabs {
    pub tabs: Vec<AppTab>,
    pub selected: usize,
}
impl Default for AppTabs {
    fn default() -> Self {
        Self::new()
    }
}

impl AppTabs {
    pub fn new() -> Self {
        Self {
            tabs: vec![
                AppTab::Overview(OverviewTab::new()),
                AppTab::Visualizer(VisualizerTab::new()),
                AppTab::Search(SearchTab::new()),
                AppTab::Errors(ErrorsTab::new()),
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
        let vertical_layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
        let [tabs_area, body_area] = vertical_layout.areas(area);

        // render tabs
        Tabs::new(self.tabs.iter().map(|t| {
            let mut line = Line::default();
            line.push_span(Span::raw(" "));
            line.push_span(t.title().fg(Color::LightBlue).bg(Color::Black));
            line.push_span(Span::raw(" "));
            line
        }))
        .highlight_style(Style::default().fg(Color::White).bg(Color::Blue))
        .select(self.selected)
        .padding("", "")
        .divider(" ")
        .render(tabs_area, buf);

        // render body border
        let content_block = Block::bordered()
            .border_set(PROPORTIONAL_TALL)
            .border_style(Color::Blue)
            .padding(Padding::horizontal(1));
        let content_inner = content_block.inner(body_area);
        content_block.render(body_area, buf);

        // render body
        self.tabs[self.selected].render(content_inner, buf, mft_files, processing_begin);
    }
}
