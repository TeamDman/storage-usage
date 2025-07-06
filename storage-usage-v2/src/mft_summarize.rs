use humantime::format_duration;
use mft::MftParser;
use mft::attribute::MftAttributeContent;
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEventKind;
use ratatui::crossterm::event::{self};
use ratatui::layout::Constraint;
use ratatui::layout::Flex;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::symbols::Marker;
use ratatui::symbols::{self};
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Clear;
use ratatui::widgets::Gauge;
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;
use ratatui::widgets::ScrollbarState;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::Tabs;
use ratatui::widgets::Widget;
use ratatui::widgets::canvas::Canvas;
use ratatui::widgets::canvas::Rectangle;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use strum::Display;
use strum::FromRepr;

#[derive(Debug, Clone)]
enum ProgressMessage {
    EntryProcessed {
        is_valid: bool,
        filename_found: Option<String>,
        attribute_type: Option<String>,
    },
    EstimatedTotal(usize),
    Complete,
    Error(Line<'static>),
    ParseError(Line<'static>),
}

#[derive(Debug, Clone)]
struct AnalysisProgress {
    total_entries: usize,
    valid_entries: usize,
    error_entries: usize,
    attribute_counts: HashMap<String, usize>,
    filename_entries: usize,
    sample_paths: Vec<String>,
    entry_statuses: Vec<bool>,

    // Progress tracking
    is_complete: bool,
    start_time: Instant,
    completion_time: Option<Instant>,
    last_update: Instant,
    entries_per_second: f64,
    estimated_total: Option<usize>,
    eta_seconds: Option<f64>,

    // Error tracking
    errors: Vec<Line<'static>>,
    critical_errors: usize,
    entry_errors: usize,
    attr_errors: usize,
}

impl Default for AnalysisProgress {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            total_entries: 0,
            valid_entries: 0,
            error_entries: 0,
            attribute_counts: HashMap::new(),
            filename_entries: 0,
            sample_paths: Vec::new(),
            entry_statuses: Vec::new(),
            is_complete: false,
            start_time: now,
            completion_time: None,
            last_update: now,
            entries_per_second: 0.0,
            estimated_total: None,
            eta_seconds: None,
            errors: Vec::new(),
            critical_errors: 0,
            entry_errors: 0,
            attr_errors: 0,
        }
    }
}

impl AnalysisProgress {
    fn update_with_message(&mut self, message: ProgressMessage) {
        match message {
            ProgressMessage::EntryProcessed {
                is_valid,
                filename_found,
                attribute_type,
            } => {
                self.total_entries += 1;
                if is_valid {
                    self.valid_entries += 1;
                } else {
                    self.error_entries += 1;
                }

                if let Some(filename) = filename_found {
                    self.filename_entries += 1;
                    if self.sample_paths.len() < 20 {
                        self.sample_paths.push(filename);
                    }
                }

                if let Some(attr_type) = attribute_type {
                    *self.attribute_counts.entry(attr_type).or_insert(0) += 1;
                }

                self.entry_statuses.push(is_valid);

                // Update rate calculation every 100 entries
                if self.total_entries.is_multiple_of(100) {
                    self.recalculate_rate_and_eta();
                }
            }
            ProgressMessage::EstimatedTotal(total) => {
                self.estimated_total = Some(total);
                self.recalculate_rate_and_eta();
            }
            ProgressMessage::Complete => {
                self.is_complete = true;
                self.completion_time = Some(Instant::now());
                self.eta_seconds = Some(0.0);
            }
            ProgressMessage::Error(error_msg) => {
                self.errors.push(error_msg.clone());
                // Count error types based on the message content
                if let Some(first_span) = error_msg.spans.get(0) {
                    let content = first_span.content.as_ref();
                    if content.contains("[CRITICAL]") {
                        self.critical_errors += 1;
                    } else if content.contains("[ENTRY]") {
                        self.entry_errors += 1;
                    } else if content.contains("[ATTR]") {
                        self.attr_errors += 1;
                    }
                }
                self.is_complete = true;
                self.completion_time = Some(Instant::now());
                self.eta_seconds = Some(0.0);
            }
            ProgressMessage::ParseError(error_msg) => {
                self.errors.push(error_msg.clone());
                // Count error types based on the message content
                if let Some(first_span) = error_msg.spans.get(0) {
                    let content = first_span.content.as_ref();
                    if content.contains("[CRITICAL]") {
                        self.critical_errors += 1;
                    } else if content.contains("[ENTRY]") {
                        self.entry_errors += 1;
                    } else if content.contains("[ATTR]") {
                        self.attr_errors += 1;
                    }
                }
                // Don't mark as complete for parse errors, just collect them
            }
        }
    }

    fn recalculate_rate_and_eta(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.start_time).as_secs_f64();
        if elapsed > 0.0 {
            self.entries_per_second = self.total_entries as f64 / elapsed;

            // Estimate ETA based on current rate
            if let Some(estimated_total) = self.estimated_total {
                let remaining = estimated_total.saturating_sub(self.total_entries);
                if self.entries_per_second > 0.0 {
                    self.eta_seconds = Some(remaining as f64 / self.entries_per_second);
                }
            }
        }
        self.last_update = now;
    }

    fn progress_percentage(&self) -> u16 {
        if let Some(estimated_total) = self.estimated_total
            && estimated_total > 0
        {
            return ((self.total_entries as f64 / estimated_total as f64) * 100.0).min(100.0)
                as u16;
        }
        0
    }

    fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    fn create_styled_error(error_type: &str, message: String) -> Line<'static> {
        match error_type.to_lowercase().as_str() {
            "attribute" => Line::from(vec!["[ATTR] ".yellow().bold(), message.white()]),
            "entry" => Line::from(vec!["[ENTRY] ".red().bold(), message.white()]),
            "critical" => Line::from(vec!["[CRITICAL] ".red().bold().on_black(), message.white()]),
            _ => Line::from(vec!["[ERROR] ".magenta().bold(), message.white()]),
        }
    }

    fn error_summary(&self) -> (usize, usize, usize) {
        (self.critical_errors, self.entry_errors, self.attr_errors)
    }
}

#[derive(Default)]
struct MftSummaryApp {
    application_state: ApplicationState,
    selected_tab: SelectedTab,
    mft_file: PathBuf,
    analysis_progress: AnalysisProgress,
    progress_receiver: Option<mpsc::Receiver<ProgressMessage>>,
    text_scroll_position: u16,
    text_scroll_state: ScrollbarState,
    last_text_area_height: u16,
    show_legend: bool,
    is_paused: bool,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum ApplicationState {
    #[default]
    Running,
    Quitting,
}

#[derive(Default, Clone, Copy, Display, FromRepr)]
enum SelectedTab {
    #[default]
    #[strum(to_string = "Progress")]
    Progress,
    #[strum(to_string = "Text Summary")]
    TextSummary,
    #[strum(to_string = "Visualization")]
    Visualization,
    #[strum(to_string = "Errors")]
    Errors,
}

impl SelectedTab {
    fn available_tabs(has_errors: bool) -> Vec<Self> {
        let mut tabs = vec![Self::Progress, Self::TextSummary, Self::Visualization];
        if has_errors {
            tabs.push(Self::Errors);
        }
        tabs
    }

    fn previous(self, has_errors: bool) -> Self {
        let available = Self::available_tabs(has_errors);
        if let Some(current_pos) = available.iter().position(|&tab| tab as u8 == self as u8) {
            let previous_pos = current_pos.saturating_sub(1);
            available[previous_pos]
        } else {
            available[0] // Default to first tab if current not found
        }
    }

    fn next(self, has_errors: bool) -> Self {
        let available = Self::available_tabs(has_errors);
        if let Some(current_pos) = available.iter().position(|&tab| tab as u8 == self as u8) {
            let next_pos = (current_pos + 1) % available.len();
            available[next_pos]
        } else {
            available[0] // Default to first tab if current not found
        }
    }

    fn title(self, progress: &AnalysisProgress, is_paused: bool) -> Line<'static> {
        let title_text = match self {
            Self::Progress if !progress.is_complete => {
                let pause_indicator = if is_paused { " [PAUSED]" } else { "" };
                format!("  Progress ({}%){pause_indicator}  ", progress.progress_percentage())
            }
            Self::Errors if progress.has_errors() => {
                format!("  Errors ({})  ", progress.errors.len())
            }
            _ => format!("  {self}  "),
        };

        title_text.fg(Color::LightBlue).bg(Color::Black).into()
    }

    fn block(self) -> Block<'static> {
        Block::bordered()
            .border_set(symbols::border::PROPORTIONAL_TALL)
            .padding(Padding::horizontal(1))
            .border_style(Color::Blue)
    }
}

impl MftSummaryApp {
    fn new(mft_file: PathBuf) -> Self {
        Self {
            mft_file,
            show_legend: false,
            is_paused: false,
            ..Default::default()
        }
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> eyre::Result<()> {
        // Clear the screen before starting the app
        terminal.clear()?;

        // Start background analysis
        let progress_receiver = self.start_background_analysis()?;
        self.progress_receiver = Some(progress_receiver);

        // Run the interactive UI
        while self.application_state == ApplicationState::Running {
            // Process any progress messages
            self.update_progress();

            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;

            // Handle events with timeout to allow for progress updates
            // Use longer timeout when there are many errors to reduce UI updates
            let poll_timeout = Duration::from_millis(100);

            if event::poll(poll_timeout)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('l') | KeyCode::Right => self.next_tab(),
                    KeyCode::Char('h')
                        if matches!(self.selected_tab, SelectedTab::Visualization) =>
                    {
                        self.show_legend = !self.show_legend;
                    }
                    KeyCode::Char('h') | KeyCode::Left => self.previous_tab(),
                    KeyCode::Char('p') => {
                        self.is_paused = !self.is_paused;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if matches!(
                            self.selected_tab,
                            SelectedTab::TextSummary | SelectedTab::Errors
                        ) {
                            let increment = if matches!(self.selected_tab, SelectedTab::Errors)
                                && self.analysis_progress.errors.len() > 100
                            {
                                // Faster scrolling for large error lists
                                2
                            } else {
                                1
                            };
                            self.text_scroll_position =
                                self.text_scroll_position.saturating_add(increment);
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if matches!(
                            self.selected_tab,
                            SelectedTab::TextSummary | SelectedTab::Errors
                        ) {
                            let increment = if matches!(self.selected_tab, SelectedTab::Errors)
                                && self.analysis_progress.errors.len() > 100
                            {
                                // Faster scrolling for large error lists
                                2
                            } else {
                                1
                            };
                            self.text_scroll_position =
                                self.text_scroll_position.saturating_sub(increment);
                        }
                    }
                    KeyCode::PageDown => {
                        if matches!(
                            self.selected_tab,
                            SelectedTab::TextSummary | SelectedTab::Errors
                        ) {
                            let page_size = if matches!(self.selected_tab, SelectedTab::Errors)
                                && self.analysis_progress.errors.len() > 100
                            {
                                // Larger page size for error lists
                                self.last_text_area_height.max(10)
                            } else {
                                (self.last_text_area_height / 2).max(1)
                            };
                            self.text_scroll_position =
                                self.text_scroll_position.saturating_add(page_size);
                        }
                    }
                    KeyCode::PageUp => {
                        if matches!(
                            self.selected_tab,
                            SelectedTab::TextSummary | SelectedTab::Errors
                        ) {
                            let page_size = if matches!(self.selected_tab, SelectedTab::Errors)
                                && self.analysis_progress.errors.len() > 100
                            {
                                // Larger page size for error lists
                                self.last_text_area_height.max(10)
                            } else {
                                (self.last_text_area_height / 2).max(1)
                            };
                            self.text_scroll_position =
                                self.text_scroll_position.saturating_sub(page_size);
                        }
                    }
                    KeyCode::Home => {
                        if matches!(
                            self.selected_tab,
                            SelectedTab::TextSummary | SelectedTab::Errors
                        ) {
                            self.text_scroll_position = 0;
                        }
                    }
                    KeyCode::End => {
                        if matches!(
                            self.selected_tab,
                            SelectedTab::TextSummary | SelectedTab::Errors
                        ) {
                            // Jump to end - for errors tab, calculate based on virtual content
                            let max_scroll = if matches!(self.selected_tab, SelectedTab::Errors) {
                                let lines_per_error = 2;
                                (self.analysis_progress.errors.len() * lines_per_error)
                                    .saturating_sub(self.last_text_area_height as usize / 2)
                                    as u16
                            } else {
                                1000 // Large number for text summary
                            };
                            self.text_scroll_position = max_scroll;
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => self.quit(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn start_background_analysis(&self) -> eyre::Result<mpsc::Receiver<ProgressMessage>> {
        let (sender, receiver) = mpsc::channel();
        let mft_file = self.mft_file.clone();

        thread::spawn(move || {
            if let Err(e) = Self::analyze_mft_in_background(mft_file, sender.clone()) {
                let error_line = AnalysisProgress::create_styled_error("critical", e.to_string());
                let _ = sender.send(ProgressMessage::Error(error_line));
            }
        });

        Ok(receiver)
    }

    fn analyze_mft_in_background(
        mft_file: PathBuf,
        sender: mpsc::Sender<ProgressMessage>,
    ) -> eyre::Result<()> {
        let mut parser = MftParser::from_path(&mft_file)?;

        // Try to estimate total entries by file size (rough estimate)
        if let Ok(metadata) = std::fs::metadata(&mft_file) {
            let file_size = metadata.len();
            // Rough estimate: 1024 bytes per entry on average
            let estimated_entries = (file_size / 1024) as usize;
            let _ = sender.send(ProgressMessage::EstimatedTotal(estimated_entries));
        }

        for entry_result in parser.iter_entries() {
            let mut filename_found = None;
            let mut attribute_type = None;

            let is_valid = match entry_result {
                Ok(entry) => {
                    // Analyze attributes
                    for attribute_result in entry.iter_attributes() {
                        match attribute_result {
                            Ok(attribute) => {
                                let attr_type = format!("{:?}", attribute.header.type_code);

                                // Only send the first attribute type we find for this entry
                                if attribute_type.is_none() {
                                    attribute_type = Some(attr_type);
                                }

                                if let MftAttributeContent::AttrX30(filename_attribute) =
                                    &attribute.data
                                {
                                    let filename = &filename_attribute.name;
                                    if !filename.is_empty()
                                        && !filename.starts_with('$')
                                        && !filename.eq(".")
                                        && filename.len() > 2
                                        && (filename.contains('.') || filename.len() > 8)
                                    {
                                        filename_found = Some(filename.clone());
                                    }
                                }
                            }
                            Err(attr_error) => {
                                // Attribute parsing error - collect it but still count entry as valid
                                let error_line = AnalysisProgress::create_styled_error(
                                    "attribute",
                                    format!("{}", attr_error),
                                );
                                let _ = sender.send(ProgressMessage::ParseError(error_line));
                            }
                        }
                    }
                    true
                }
                Err(entry_error) => {
                    // Entry parsing error
                    let error_line =
                        AnalysisProgress::create_styled_error("entry", format!("{}", entry_error));
                    let _ = sender.send(ProgressMessage::ParseError(error_line));
                    false
                }
            };

            // Send progress update
            if sender
                .send(ProgressMessage::EntryProcessed {
                    is_valid,
                    filename_found,
                    attribute_type,
                })
                .is_err()
            {
                break; // Receiver dropped, stop processing
            }
        }

        let _ = sender.send(ProgressMessage::Complete);
        Ok(())
    }

    fn update_progress(&mut self) {
        if !self.is_paused {
            if let Some(receiver) = &self.progress_receiver {
                // Process all available messages without blocking
                while let Ok(message) = receiver.try_recv() {
                    self.analysis_progress.update_with_message(message);
                }
            }
        }
    }

    fn next_tab(&mut self) {
        let has_errors = self.analysis_progress.has_errors();
        self.selected_tab = self.selected_tab.next(has_errors);
        // Reset scroll position when changing tabs
        self.text_scroll_position = 0;
    }

    fn previous_tab(&mut self) {
        let has_errors = self.analysis_progress.has_errors();
        self.selected_tab = self.selected_tab.previous(has_errors);
        // Reset scroll position when changing tabs
        self.text_scroll_position = 0;
    }

    fn quit(&mut self) {
        self.application_state = ApplicationState::Quitting;
    }
}

impl Widget for &mut MftSummaryApp {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        use Constraint::Length;
        use Constraint::Min;
        let vertical_layout = Layout::vertical([Length(1), Min(0), Length(1)]);
        let [header_area, inner_area, footer_area] = vertical_layout.areas(area);

        // Split header to properly allocate space - tabs get minimum needed, file path gets rest
        let horizontal_layout = Layout::horizontal([Constraint::Min(30), Constraint::Length(30)]);
        let [tabs_area, title_area] = horizontal_layout.areas(header_area);

        self.render_tabs(tabs_area, buffer);
        self.render_title(title_area, buffer);

        // Store area height for page scrolling
        self.last_text_area_height = inner_area.height;

        self.selected_tab.render_content(
            &self.analysis_progress,
            inner_area,
            buffer,
            self.text_scroll_position,
            &mut self.text_scroll_state,
            self.show_legend,
        );
        self.render_footer(footer_area, buffer);

        #[cfg(debug_assertions)]
        {
            // Render debug dimensions at bottom right
            let dims = format!("{}x{}", area.width, area.height);
            let debug_style = ratatui::style::Style::default().fg(Color::Gray);
            buffer.set_string(
                area.x + area.width.saturating_sub(dims.len() as u16),
                area.y + area.height - 1,
                &dims,
                debug_style,
            );
        }
    }
}

impl MftSummaryApp {
    fn render_title(&self, area: Rect, buffer: &mut Buffer) {
        let filename = self
            .mft_file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        // Truncate filename if it's too long for the area
        let max_width = area.width.saturating_sub(6) as usize; // "File: " = 6 chars
        let display_filename = if filename.len() > max_width {
            let start_pos = filename.len().saturating_sub(max_width.saturating_sub(3));
            format!("...{}", &filename[start_pos..])
        } else {
            filename.to_string()
        };

        Line::from(format!("File: {display_filename}"))
            .style(Color::Yellow)
            .right_aligned()
            .render(area, buffer);
    }

    fn render_tabs(&self, area: Rect, buffer: &mut Buffer) {
        let has_errors = self.analysis_progress.has_errors();
        let available_tabs = SelectedTab::available_tabs(has_errors);
        let titles = available_tabs
            .iter()
            .map(|tab| tab.title(&self.analysis_progress, self.is_paused));
        let highlight_style = (Color::White, Color::Blue);

        // Find the index of the selected tab in the available tabs list
        let selected_tab_index = available_tabs
            .iter()
            .position(|&tab| tab as u8 == self.selected_tab as u8)
            .unwrap_or(0);

        Tabs::new(titles)
            .highlight_style(highlight_style)
            .select(selected_tab_index)
            .padding("", "")
            .divider(" ")
            .render(area, buffer);
    }

    fn render_footer(&self, area: Rect, buffer: &mut Buffer) {
        // Determine base footer text
        let base_footer = if matches!(
            self.selected_tab,
            SelectedTab::TextSummary | SelectedTab::Errors
        ) {
            if matches!(self.selected_tab, SelectedTab::Errors)
                && self.analysis_progress.errors.len() > 50
            {
                "◄ ► tab | ▲ ▼ scroll | PgUp PgDn fast | Home End jump | p pause | q quit"
            } else {
                "◄ ► to change tab | ▲ ▼ to scroll | PgUp PgDn for fast scroll | p pause | q quit"
            }
        } else if matches!(self.selected_tab, SelectedTab::Visualization) {
            "◄ ► to change tab | h to toggle legend | p pause | q quit"
        } else {
            "◄ ► to change tab | p pause | q quit"
        };

        // Add pause status if paused
        let footer_text = if self.is_paused {
            format!("{} | [PAUSED]", base_footer)
        } else {
            base_footer.to_string()
        };

        Line::raw(footer_text).centered().render(area, buffer);
    }
}

impl SelectedTab {
    fn render_content(
        self,
        progress: &AnalysisProgress,
        area: Rect,
        buffer: &mut Buffer,
        text_scroll_position: u16,
        scroll_state: &mut ScrollbarState,
        show_legend: bool,
    ) {
        match self {
            Self::Progress => self.render_progress_tab(progress, area, buffer),
            Self::TextSummary => {
                self.render_text_summary(progress, area, buffer, text_scroll_position, scroll_state)
            }
            Self::Visualization => self.render_visualization(progress, area, buffer, show_legend),
            Self::Errors => {
                self.render_errors_tab(progress, area, buffer, text_scroll_position, scroll_state)
            }
        }
    }

    fn render_text_summary(
        self,
        progress: &AnalysisProgress,
        area: Rect,
        buffer: &mut Buffer,
        text_scroll_position: u16,
        scroll_state: &mut ScrollbarState,
    ) {
        let mut text_content = Vec::new();

        text_content.push(Line::from("=== MFT Summary ===".bold().yellow()));
        text_content.push(Line::from(""));

        text_content.push(Line::from("Entry Statistics:".bold().cyan()));

        // Show real-time stats with rates
        let entries_rate = if progress.entries_per_second > 0.0 {
            format!(" (+{:.0} entries/s)", progress.entries_per_second).green()
        } else {
            "".into()
        };

        text_content.push(Line::from(vec![
            "  Total entries processed: ".into(),
            progress.total_entries.to_string().bold().white(),
            entries_rate,
        ]));
        text_content.push(Line::from(vec![
            "  Valid entries: ".into(),
            progress.valid_entries.to_string().green(),
        ]));
        text_content.push(Line::from(vec![
            "  Error entries: ".into(),
            progress.error_entries.to_string().red(),
        ]));

        let success_rate = if progress.total_entries > 0 {
            (progress.valid_entries as f64 / progress.total_entries as f64) * 100.0
        } else {
            0.0
        };
        let rate_color = if success_rate > 80.0 {
            Color::Green
        } else if success_rate > 50.0 {
            Color::Yellow
        } else {
            Color::Red
        };
        text_content.push(Line::from(vec![
            "  Success rate: ".into(),
            format!("{success_rate:.2}%").fg(rate_color).bold(),
        ]));

        // Show ETA if available
        if let Some(eta) = progress.eta_seconds
            && eta > 0.0
            && !progress.is_complete
        {
            let eta_duration = Duration::from_secs_f64(eta);
            text_content.push(Line::from(vec![
                "  ETA: ".into(),
                format_duration(eta_duration).to_string().magenta().bold(),
            ]));
        }

        text_content.push(Line::from(""));

        text_content.push(Line::from("File System Structure:".bold().cyan()));
        text_content.push(Line::from(vec![
            "  Filename attributes: ".into(),
            progress.filename_entries.to_string().magenta(),
        ]));
        text_content.push(Line::from(""));

        text_content.push(Line::from("Attribute Statistics:".bold().cyan()));
        text_content.push(Line::from(vec![
            "  Total attributes found: ".into(),
            progress
                .attribute_counts
                .values()
                .sum::<usize>()
                .to_string()
                .white()
                .bold(),
        ]));

        if !progress.attribute_counts.is_empty() {
            text_content.push(Line::from("  Top attribute types:".blue()));
            let mut sorted_attributes: Vec<_> = progress.attribute_counts.iter().collect();
            sorted_attributes.sort_by(|a, b| b.1.cmp(a.1));

            for (attr_type, count) in sorted_attributes.iter().take(5) {
                text_content.push(Line::from(vec![
                    "    ".into(),
                    format!("{attr_type}: ").white(),
                    count.to_string().yellow(),
                ]));
            }
        }

        text_content.push(Line::from(""));

        if !progress.sample_paths.is_empty() {
            text_content.push(Line::from(
                format!(
                    "Sample File Paths (first {} found):",
                    progress.sample_paths.len().min(10)
                )
                .bold()
                .cyan(),
            ));
            for (i, path) in progress.sample_paths.iter().take(10).enumerate() {
                text_content.push(Line::from(vec![
                    format!("  {}: ", i + 1).blue(),
                    path.clone().light_blue(),
                ]));
            }
        }

        if !progress.is_complete {
            text_content.push(Line::from(""));
            text_content.push(Line::from("Analysis in progress...".yellow().italic()));
        } else {
            text_content.push(Line::from(""));
            text_content.push(Line::from("✓ Analysis complete!".green().bold()));
        }

        // Calculate content height for scrollbar
        let content_height = text_content.len() as u16;
        let visible_height = area.height.saturating_sub(2); // Account for block borders

        // Update scrollbar state
        *scroll_state = scroll_state
            .content_length(content_height as usize)
            .viewport_content_length(visible_height as usize)
            .position(text_scroll_position as usize);

        // Create layout for paragraph and scrollbar
        let horizontal_layout = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]);
        let [text_area, scrollbar_area] = horizontal_layout.areas(area);

        Paragraph::new(Text::from(text_content))
            .block(self.block())
            .scroll((text_scroll_position, 0))
            .render(text_area, buffer);

        // Render scrollbar if content is scrollable
        if content_height > visible_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");
            StatefulWidget::render(scrollbar, scrollbar_area, buffer, scroll_state);
        }
    }

    fn render_visualization(
        self,
        progress: &AnalysisProgress,
        area: Rect,
        buffer: &mut Buffer,
        show_legend: bool,
    ) {
        if progress.entry_statuses.is_empty() {
            Paragraph::new("No data available yet. Processing...")
                .block(self.block())
                .render(area, buffer);
            return;
        }

        // Calculate actual canvas dimensions based on the inner area
        let inner_area = self.block().inner(area);
        let canvas_width = inner_area.width as f64;
        let canvas_height = inner_area.height as f64;
        
        Canvas::default()
            .block(self.block())
            .marker(Marker::Block)
            .x_bounds([0.0, canvas_width])
            .y_bounds([0.0, canvas_height])
            .paint(|context| {
                let total_pixels = (canvas_width * canvas_height) as usize;

                let entries_per_pixel = if total_pixels > 0 {
                    (progress.entry_statuses.len() as f64 / total_pixels as f64).ceil() as usize
                } else {
                    1
                };
                let entries_per_pixel = entries_per_pixel.max(1);

                // Pre-calculate pixel data in a more efficient way
                let mut pixel_data = Vec::with_capacity(total_pixels);
                
                for pixel_index in 0..total_pixels {
                    let start_entry = pixel_index * entries_per_pixel;
                    let end_entry = ((pixel_index + 1) * entries_per_pixel).min(progress.entry_statuses.len());

                    if start_entry >= progress.entry_statuses.len() {
                        pixel_data.push((Color::Black, 10)); // Use 10 for empty pixels
                        continue;
                    }

                    let mut valid_count = 0;
                    let mut invalid_count = 0;

                    for entry_index in start_entry..end_entry {
                        if progress.entry_statuses[entry_index] {
                            valid_count += 1;
                        } else {
                            invalid_count += 1;
                        }
                    }

                    let total_entries_in_pixel = valid_count + invalid_count;
                    let (color, quality_score) = if total_entries_in_pixel == 0 {
                        (Color::Black, 10) // Use 10 for empty pixels
                    } else {
                        let valid_ratio = valid_count as f64 / total_entries_in_pixel as f64;
                        (Self::get_quality_color(valid_ratio), Self::get_quality_score(valid_ratio))
                    };

                    pixel_data.push((color, quality_score));
                }

                // Render pixels and detect region boundaries for numbering
                let width_usize = canvas_width as usize;
                
                for (pixel_index, &(color, _quality_score)) in pixel_data.iter().enumerate() {
                    let pixel_x = pixel_index % width_usize;
                    let pixel_y = pixel_index / width_usize;

                    // Draw the pixel - flip Y coordinate for proper orientation
                    context.draw(&Rectangle {
                        x: pixel_x as f64,
                        y: (canvas_height as usize - pixel_y - 1) as f64,
                        width: 1.0,
                        height: 1.0,
                        color,
                    });
                }

                // Second pass: find region start positions and place numbers
                for (pixel_index, &(color, quality_score)) in pixel_data.iter().enumerate() {
                    if color == Color::Black || quality_score == 10 {
                        continue; // Skip empty pixels
                    }

                    let pixel_x = pixel_index % width_usize;
                    let pixel_y = pixel_index / width_usize;

                    // Check if this is the top-left corner of a new region
                    let is_region_start = Self::is_region_start(&pixel_data, pixel_index, width_usize, canvas_height as usize, quality_score);

                    if is_region_start {
                        let text_color = if quality_score <= 2 {
                            Color::Black // Dark text on bright colors
                        } else {
                            Color::White // Light text on dark colors
                        };

                        context.print(
                            pixel_x as f64,
                            (canvas_height as usize - pixel_y - 1) as f64,
                            quality_score.to_string().fg(text_color).bold(),
                        );
                    }
                }
            })
            .render(area, buffer);

        // Render legend as a popup overlay if show_legend is true
        if show_legend {
            self.render_legend_popup(progress, area, buffer);
        }
    }

    fn render_progress_tab(self, progress: &AnalysisProgress, area: Rect, buffer: &mut Buffer) {
        let mut content = Vec::new();

        content.push(Line::from("=== Analysis Progress ==="));
        content.push(Line::from(""));

        // Progress bar
        if let Some(estimated_total) = progress.estimated_total {
            let progress_ratio = if estimated_total > 0 {
                progress.total_entries as f64 / estimated_total as f64
            } else {
                0.0
            };

            content.push(Line::from(format!(
                "Progress: {}/{} entries ({:.1}%)",
                progress.total_entries,
                estimated_total,
                progress_ratio * 100.0
            )));
        } else {
            content.push(Line::from(format!(
                "Progress: {} entries processed",
                progress.total_entries
            )));
        }

        content.push(Line::from(""));

        // Performance metrics
        content.push(Line::from("Performance:"));
        content.push(Line::from(format!(
            "  Processing rate: {:.0} entries/second",
            progress.entries_per_second
        )));

        let elapsed = if let Some(completion_time) = progress.completion_time {
            completion_time.duration_since(progress.start_time)
        } else {
            progress.start_time.elapsed()
        };
        content.push(Line::from(format!(
            "  Elapsed time: {}",
            format_duration(elapsed)
        )));

        if let Some(eta) = progress.eta_seconds
            && eta > 0.0
            && !progress.is_complete
        {
            let eta_duration = Duration::from_secs_f64(eta);
            content.push(Line::from(format!(
                "  Estimated time remaining: {}",
                format_duration(eta_duration)
            )));
        }

        content.push(Line::from(""));

        // Statistics
        content.push(Line::from("Current Statistics:"));
        content.push(Line::from(format!(
            "  Valid entries: {} ({:.1}%)",
            progress.valid_entries,
            if progress.total_entries > 0 {
                progress.valid_entries as f64 / progress.total_entries as f64 * 100.0
            } else {
                0.0
            }
        )));
        content.push(Line::from(format!(
            "  Error entries: {} ({:.1}%)",
            progress.error_entries,
            if progress.total_entries > 0 {
                progress.error_entries as f64 / progress.total_entries as f64 * 100.0
            } else {
                0.0
            }
        )));
        content.push(Line::from(format!(
            "  Filename attributes found: {}",
            progress.filename_entries
        )));

        if progress.is_complete {
            content.push(Line::from(""));
            content.push(Line::from("Analysis complete!").green());
        }

        // Add a simple gauge for visual progress
        let gauge_area = if area.height > 15 {
            let layout = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]);
            let [text_area, gauge_area] = layout.areas(area);

            Paragraph::new(Text::from(content))
                .block(self.block())
                .render(text_area, buffer);

            Some(gauge_area)
        } else {
            Paragraph::new(Text::from(content))
                .block(self.block())
                .render(area, buffer);
            None
        };

        if let Some(gauge_area) = gauge_area {
            let progress_ratio = if let Some(estimated_total) = progress.estimated_total {
                if estimated_total > 0 {
                    (progress.total_entries as f64 / estimated_total as f64).min(1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };

            Gauge::default()
                .block(Block::bordered().title("Progress"))
                .gauge_style(Color::Green)
                .ratio(progress_ratio)
                .render(gauge_area, buffer);
        }
    }

    fn render_errors_tab(
        self,
        progress: &AnalysisProgress,
        area: Rect,
        buffer: &mut Buffer,
        text_scroll_position: u16,
        scroll_state: &mut ScrollbarState,
    ) {
        let mut text_content = Vec::new();

        text_content.push(Line::from("=== Errors ===".bold().red()));
        text_content.push(Line::from(""));

        if progress.errors.is_empty() {
            text_content.push(Line::from("No errors recorded.".green()));

            // Simple rendering for no errors case
            Paragraph::new(Text::from(text_content))
                .block(self.block())
                .render(area, buffer);
            return;
        }

        // Show error count and categorization
        text_content.push(Line::from(
            format!("Total errors encountered: {}", progress.errors.len())
                .red()
                .bold(),
        ));

        let (critical_count, entry_count, attr_count) = progress.error_summary();
        if critical_count > 0 || entry_count > 0 || attr_count > 0 {
            text_content.push(Line::from("Error breakdown:".cyan()));
            if critical_count > 0 {
                text_content.push(Line::from(vec![
                    "  Critical: ".red().bold(),
                    critical_count.to_string().white(),
                ]));
            }
            if entry_count > 0 {
                text_content.push(Line::from(vec![
                    "  Entry parsing: ".red(),
                    entry_count.to_string().white(),
                ]));
            }
            if attr_count > 0 {
                text_content.push(Line::from(vec![
                    "  Attribute parsing: ".yellow(),
                    attr_count.to_string().white(),
                ]));
            }
        }
        text_content.push(Line::from(""));

        // Virtual scrolling implementation - optimized for performance
        let visible_height = area.height.saturating_sub(2); // Account for block borders
        let header_lines = text_content.len() as u16; // Header content we always show
        let available_content_height = visible_height.saturating_sub(header_lines);

        // Each error takes 2 lines (error + empty line)
        let lines_per_error = 2;
        let visible_errors = (available_content_height / lines_per_error) as usize;

        // Calculate which errors to show based on scroll position
        let start_error = (text_scroll_position / lines_per_error) as usize;
        let end_error = (start_error + visible_errors).min(progress.errors.len());

        // Limit errors to avoid performance issues - show max 1000 errors at once
        let max_errors_to_show = 1000;
        let actual_end = end_error.min(start_error + max_errors_to_show);

        // Add visible errors with complete error messages
        for i in start_error..actual_end {
            if let Some(error) = progress.errors.get(i) {
                // Create numbered error line
                let error_number = format!("{}. ", i + 1).red().bold();

                // Reconstruct the complete error message from spans
                let mut error_message = String::new();
                for span in &error.spans {
                    error_message.push_str(&span.content);
                }

                // Create the complete line with number and message
                let line_parts = vec![error_number, error_message.white()];

                text_content.push(Line::from(line_parts));
                text_content.push(Line::from(""));
            }
        }

        // Show scrolling info if there are more errors
        if progress.errors.len() > visible_errors {
            text_content.push(Line::from(""));
            let showing_end = actual_end.min(progress.errors.len());
            if progress.errors.len() > max_errors_to_show {
                text_content.push(Line::from(
                    format!(
                        "Showing errors {}-{} of {} (limited to {} for performance)",
                        start_error + 1,
                        showing_end,
                        progress.errors.len(),
                        max_errors_to_show
                    )
                    .cyan()
                    .italic(),
                ));
            } else {
                text_content.push(Line::from(
                    format!(
                        "Showing errors {}-{} of {} (use ↑↓ or PgUp/PgDn to scroll)",
                        start_error + 1,
                        showing_end,
                        progress.errors.len()
                    )
                    .cyan()
                    .italic(),
                ));
            }
        }

        // Calculate total virtual content height for scrollbar
        let total_content_height =
            header_lines + (progress.errors.len() * lines_per_error as usize) as u16;

        // Update scrollbar state with virtual content
        *scroll_state = scroll_state
            .content_length(total_content_height as usize)
            .viewport_content_length(visible_height as usize)
            .position(text_scroll_position as usize);

        // Create layout for paragraph and scrollbar
        let horizontal_layout = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]);
        let [text_area, scrollbar_area] = horizontal_layout.areas(area);

        Paragraph::new(Text::from(text_content))
            .block(self.block())
            .render(text_area, buffer);

        // Render scrollbar if content is scrollable
        if total_content_height > visible_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");
            StatefulWidget::render(scrollbar, scrollbar_area, buffer, scroll_state);
        }
    }

    // Helper function to create a popup area
    fn popup_area(area: Rect, percent_x: u16, length_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Length(length_y)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }

    // Render the legend as a popup overlay
    fn render_legend_popup(self, progress: &AnalysisProgress, area: Rect, buffer: &mut Buffer) {
        // Create popup area - larger for detailed view, smaller for compact view
        let (popup_width, popup_height) = (70,5);

        let popup_area = Self::popup_area(area, popup_width, popup_height);

        // Clear the background
        Clear.render(popup_area, buffer);

        // Create legend content
        let mut legend_content = Vec::new();

        legend_content.push(Line::from(vec![
            "Quality: 0-2=".into(),
            "Good ".green(),
            "3-6=".into(),
            "Fair ".yellow(),
            "7-9=".into(),
            "Poor".red(),
        ]));
        legend_content.push(Line::from(format!(
            "Total: {} | Valid: {} | Invalid: {}",
            progress.total_entries, progress.valid_entries, progress.error_entries
        )));
        legend_content.push(Line::from("Press h to hide".cyan()));

        // Render the popup with legend content
        Paragraph::new(Text::from(legend_content))
            .block(Block::bordered().title("Legend").border_style(Color::Blue))
            .render(popup_area, buffer);
    }
}

impl SelectedTab {
    // Helper function to get high-fidelity color based on valid ratio
    fn get_quality_color(valid_ratio: f64) -> Color {
        if valid_ratio >= 1.0 {
            Color::Rgb(0, 255, 0) // Bright green (100% valid)
        } else if valid_ratio >= 0.95 {
            Color::Rgb(50, 255, 50) // Light green (95-99% valid)
        } else if valid_ratio >= 0.9 {
            Color::Rgb(100, 255, 100) // Pale green (90-94% valid)
        } else if valid_ratio >= 0.8 {
            Color::Rgb(200, 255, 100) // Yellow-green (80-89% valid)
        } else if valid_ratio >= 0.7 {
            Color::Rgb(255, 255, 0) // Yellow (70-79% valid)
        } else if valid_ratio >= 0.6 {
            Color::Rgb(255, 200, 0) // Orange-yellow (60-69% valid)
        } else if valid_ratio >= 0.5 {
            Color::Rgb(255, 150, 0) // Orange (50-59% valid)
        } else if valid_ratio >= 0.4 {
            Color::Rgb(255, 100, 0) // Red-orange (40-49% valid)
        } else if valid_ratio >= 0.3 {
            Color::Rgb(255, 50, 0) // Red-orange (30-39% valid)
        } else if valid_ratio >= 0.1 {
            Color::Rgb(255, 0, 50) // Red (10-29% valid)
        } else if valid_ratio > 0.0 {
            Color::Rgb(200, 0, 0) // Dark red (1-9% valid)
        } else {
            Color::Rgb(150, 0, 0) // Very dark red (0% valid)
        }
    }

    // Helper function to get quality score (0-9) based on valid ratio
    fn get_quality_score(valid_ratio: f64) -> u8 {
        if valid_ratio >= 1.0 {
            0
        } else if valid_ratio >= 0.95 {
            1
        } else if valid_ratio >= 0.9 {
            2
        } else if valid_ratio >= 0.8 {
            3
        } else if valid_ratio >= 0.7 {
            4
        } else if valid_ratio >= 0.6 {
            5
        } else if valid_ratio >= 0.5 {
            6
        } else if valid_ratio >= 0.3 {
            7
        } else if valid_ratio >= 0.1 {
            8
        } else {
            9
        }
    }

    // Helper function to determine if a pixel is the start of a new region (top-left corner)
    fn is_region_start(
        pixel_data: &[(Color, u8)],
        pixel_index: usize,
        width: usize,
        _height: usize,
        current_quality: u8,
    ) -> bool {
        let x = pixel_index % width;
        let y = pixel_index / width;

        // Check if pixel above has different quality (or is out of bounds)
        let above_different = if y == 0 {
            true // Top row
        } else {
            let above_index = (y - 1) * width + x;
            pixel_data.get(above_index).map_or(true, |&(_, q)| q != current_quality)
        };

        // Check if pixel to the left has different quality (or is out of bounds)
        let left_different = if x == 0 {
            true // Left column
        } else {
            let left_index = y * width + (x - 1);
            pixel_data.get(left_index).map_or(true, |&(_, q)| q != current_quality)
        };

        // It's a region start if both above and left are different quality (or boundaries)
        above_different && left_different
    }
}

pub fn summarize_mft_file(
    mft_file: PathBuf,
    _verbose: bool,
    _show_paths: bool,
    _max_entries: Option<usize>,
) -> eyre::Result<()> {
    let terminal = ratatui::init();
    let application_result = MftSummaryApp::new(mft_file).run(terminal);
    ratatui::restore();
    application_result
}
