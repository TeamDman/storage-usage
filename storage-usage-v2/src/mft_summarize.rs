use mft::MftParser;
use mft::attribute::MftAttributeContent;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{Color, Stylize},
    symbols::{self, Marker},
    text::{Line, Text},
    widgets::{Block, Padding, Paragraph, Tabs, Widget, Gauge, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, canvas::{Canvas, Rectangle}},
    buffer::Buffer,
    DefaultTerminal,
};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};
use humantime::format_duration;

#[derive(Debug, Clone)]
enum ProgressMessage {
    EntryProcessed {
        is_valid: bool,
        filename_found: Option<String>,
        attribute_type: Option<String>,
    },
    BatchComplete {
        batch_size: usize,
        elapsed_ms: u64,
    },
    EstimatedTotal(usize),
    Complete,
    Error(String),
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
        }
    }
}

impl AnalysisProgress {
    fn update_with_message(&mut self, message: ProgressMessage) {
        match message {
            ProgressMessage::EntryProcessed { is_valid, filename_found, attribute_type } => {
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
                if self.total_entries % 100 == 0 {
                    self.update_rate();
                }
            }
            ProgressMessage::BatchComplete { batch_size: _, elapsed_ms: _ } => {
                self.update_rate();
            }
            ProgressMessage::EstimatedTotal(total) => {
                self.estimated_total = Some(total);
                self.update_rate();
            }
            ProgressMessage::Complete => {
                self.is_complete = true;
                self.completion_time = Some(Instant::now());
                self.eta_seconds = Some(0.0);
            }
            ProgressMessage::Error(_) => {
                self.is_complete = true;
                self.completion_time = Some(Instant::now());
                self.eta_seconds = Some(0.0);
            }
        }
    }
    
    fn update_rate(&mut self) {
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
        if let Some(estimated_total) = self.estimated_total {
            if estimated_total > 0 {
                return ((self.total_entries as f64 / estimated_total as f64) * 100.0).min(100.0) as u16;
            }
        }
        0
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
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum ApplicationState {
    #[default]
    Running,
    Quitting,
}

#[derive(Default, Clone, Copy, Display, FromRepr, EnumIter)]
enum SelectedTab {
    #[default]
    #[strum(to_string = "Progress")]
    Progress,
    #[strum(to_string = "Text Summary")]
    TextSummary,
    #[strum(to_string = "Visualization")]
    Visualization,
}

impl SelectedTab {
    fn previous(self) -> Self {
        let current_index: usize = self as usize;
        let previous_index = current_index.saturating_sub(1);
        Self::from_repr(previous_index).unwrap_or(self)
    }

    fn next(self) -> Self {
        let current_index = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(self)
    }

    fn title(self, progress: &AnalysisProgress) -> Line<'static> {
        let title_text = match self {
            Self::Progress if !progress.is_complete => {
                format!("  Progress ({}%)  ", progress.progress_percentage())
            }
            _ => format!("  {self}  ")
        };
        
        title_text
            .fg(Color::LightBlue)
            .bg(Color::Black)
            .into()
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
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('l') | KeyCode::Right => self.next_tab(),
                            KeyCode::Char('h') | KeyCode::Left => self.previous_tab(),
                            KeyCode::Char('j') | KeyCode::Down => {
                                if matches!(self.selected_tab, SelectedTab::TextSummary) {
                                    self.text_scroll_position = self.text_scroll_position.saturating_add(1);
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if matches!(self.selected_tab, SelectedTab::TextSummary) {
                                    self.text_scroll_position = self.text_scroll_position.saturating_sub(1);
                                }
                            }
                            KeyCode::PageDown => {
                                if matches!(self.selected_tab, SelectedTab::TextSummary) {
                                    let page_size = (self.last_text_area_height / 2).max(1);
                                    self.text_scroll_position = self.text_scroll_position.saturating_add(page_size);
                                }
                            }
                            KeyCode::PageUp => {
                                if matches!(self.selected_tab, SelectedTab::TextSummary) {
                                    let page_size = (self.last_text_area_height / 2).max(1);
                                    self.text_scroll_position = self.text_scroll_position.saturating_sub(page_size);
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Esc => self.quit(),
                            _ => {}
                        }
                    }
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
                let _ = sender.send(ProgressMessage::Error(e.to_string()));
            }
        });
        
        Ok(receiver)
    }

    fn analyze_mft_in_background(mft_file: PathBuf, sender: mpsc::Sender<ProgressMessage>) -> eyre::Result<()> {
        let mut parser = MftParser::from_path(&mft_file)?;
        
        // Try to estimate total entries by file size (rough estimate)
        if let Ok(metadata) = std::fs::metadata(&mft_file) {
            let file_size = metadata.len();
            // Rough estimate: 1024 bytes per entry on average
            let estimated_entries = (file_size / 1024) as usize;
            let _ = sender.send(ProgressMessage::EstimatedTotal(estimated_entries));
        }
        
        let batch_start = Instant::now();
        let mut batch_count = 0;
        
        for (_entry_index, entry_result) in parser.iter_entries().enumerate() {
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

                                match &attribute.data {
                                    MftAttributeContent::AttrX30(filename_attribute) => {
                                        let filename = &filename_attribute.name;
                                        if !filename.is_empty() 
                                            && !filename.starts_with('$') 
                                            && !filename.eq(".") 
                                            && filename.len() > 2 
                                            && (filename.contains('.') || filename.len() > 8) {
                                            filename_found = Some(filename.clone());
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Err(_) => {
                                // Attribute parsing error - still count the entry as valid
                            }
                        }
                    }
                    true
                }
                Err(_) => false
            };

            // Send progress update
            if sender.send(ProgressMessage::EntryProcessed { 
                is_valid, 
                filename_found,
                attribute_type 
            }).is_err() {
                break; // Receiver dropped, stop processing
            }
            
            batch_count += 1;
            
            // Send batch complete every 1000 entries
            if batch_count >= 1000 {
                let elapsed = batch_start.elapsed().as_millis() as u64;
                let _ = sender.send(ProgressMessage::BatchComplete { 
                    batch_size: batch_count, 
                    elapsed_ms: elapsed 
                });
                batch_count = 0;
            }
        }
        
        let _ = sender.send(ProgressMessage::Complete);
        Ok(())
    }

    fn update_progress(&mut self) {
        if let Some(receiver) = &self.progress_receiver {
            // Process all available messages without blocking
            while let Ok(message) = receiver.try_recv() {
                self.analysis_progress.update_with_message(message);
            }
        }
    }

    fn next_tab(&mut self) {
        self.selected_tab = self.selected_tab.next();
    }

    fn previous_tab(&mut self) {
        self.selected_tab = self.selected_tab.previous();
    }

    fn quit(&mut self) {
        self.application_state = ApplicationState::Quitting;
    }
}

impl Widget for &mut MftSummaryApp {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        use Constraint::{Length, Min};
        let vertical_layout = Layout::vertical([Length(1), Min(0), Length(1)]);
        let [header_area, inner_area, footer_area] = vertical_layout.areas(area);

        // Split header to give more space for the file path
        let horizontal_layout = Layout::horizontal([Length(60), Min(0)]);
        let [tabs_area, title_area] = horizontal_layout.areas(header_area);

        self.render_title(title_area, buffer);
        self.render_tabs(tabs_area, buffer);
        
        // Store area height for page scrolling
        self.last_text_area_height = inner_area.height;
        
        self.selected_tab.render_content(&self.analysis_progress, inner_area, buffer, self.text_scroll_position, &mut self.text_scroll_state);
        self.render_footer(footer_area, buffer);
    }
}

impl MftSummaryApp {
    fn render_title(&self, area: Rect, buffer: &mut Buffer) {
        let filename = self.mft_file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");
        
        format!("File: {}", filename)
            .fg(Color::Yellow)
            .render(area, buffer);
    }

    fn render_tabs(&self, area: Rect, buffer: &mut Buffer) {
        let titles = SelectedTab::iter().map(|tab| tab.title(&self.analysis_progress));
        let highlight_style = (Color::White, Color::Blue);
        let selected_tab_index = self.selected_tab as usize;
        Tabs::new(titles)
            .highlight_style(highlight_style)
            .select(selected_tab_index)
            .padding("", "")
            .divider(" ")
            .render(area, buffer);
    }

    fn render_footer(&self, area: Rect, buffer: &mut Buffer) {
        let footer_text = if matches!(self.selected_tab, SelectedTab::TextSummary) {
            "◄ ► to change tab | ▲ ▼ to scroll | PgUp PgDn for fast scroll | Press q to quit"
        } else {
            "◄ ► to change tab | Press q to quit"
        };
        
        Line::raw(footer_text)
            .centered()
            .render(area, buffer);
    }
}

impl SelectedTab {
    fn render_content(self, progress: &AnalysisProgress, area: Rect, buffer: &mut Buffer, text_scroll_position: u16, scroll_state: &mut ScrollbarState) {
        match self {
            Self::Progress => self.render_progress_tab(progress, area, buffer),
            Self::TextSummary => self.render_text_summary(progress, area, buffer, text_scroll_position, scroll_state),
            Self::Visualization => self.render_visualization(progress, area, buffer),
        }
    }

    fn render_text_summary(self, progress: &AnalysisProgress, area: Rect, buffer: &mut Buffer, text_scroll_position: u16, scroll_state: &mut ScrollbarState) {
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
            entries_rate.into()
        ]));
        text_content.push(Line::from(vec![
            "  Valid entries: ".into(),
            progress.valid_entries.to_string().green()
        ]));
        text_content.push(Line::from(vec![
            "  Error entries: ".into(),
            progress.error_entries.to_string().red()
        ]));
        
        let success_rate = if progress.total_entries > 0 {
            (progress.valid_entries as f64 / progress.total_entries as f64) * 100.0
        } else {
            0.0
        };
        let rate_color = if success_rate > 80.0 { Color::Green } else if success_rate > 50.0 { Color::Yellow } else { Color::Red };
        text_content.push(Line::from(vec![
            "  Success rate: ".into(),
            format!("{:.2}%", success_rate).fg(rate_color).bold()
        ]));
        
        // Show ETA if available
        if let Some(eta) = progress.eta_seconds {
            if eta > 0.0 && !progress.is_complete {
                let eta_duration = Duration::from_secs_f64(eta);
                text_content.push(Line::from(vec![
                    "  ETA: ".into(),
                    format_duration(eta_duration).to_string().magenta().bold()
                ]));
            }
        }
        
        text_content.push(Line::from(""));
        
        text_content.push(Line::from("File System Structure:".bold().cyan()));
        text_content.push(Line::from(vec![
            "  Filename attributes: ".into(),
            progress.filename_entries.to_string().magenta()
        ]));
        text_content.push(Line::from(""));
        
        text_content.push(Line::from("Attribute Statistics:".bold().cyan()));
        text_content.push(Line::from(vec![
            "  Total attributes found: ".into(),
            progress.attribute_counts.values().sum::<usize>().to_string().white().bold()
        ]));
        
        if !progress.attribute_counts.is_empty() {
            text_content.push(Line::from("  Top attribute types:".blue()));
            let mut sorted_attributes: Vec<_> = progress.attribute_counts.iter().collect();
            sorted_attributes.sort_by(|a, b| b.1.cmp(a.1));
            
            for (attr_type, count) in sorted_attributes.iter().take(5) {
                text_content.push(Line::from(vec![
                    "    ".into(),
                    format!("{}: ", attr_type).white(),
                    count.to_string().yellow()
                ]));
            }
        }
        
        text_content.push(Line::from(""));

        if !progress.sample_paths.is_empty() {
            text_content.push(Line::from(format!("Sample File Paths (first {} found):", 
                progress.sample_paths.len().min(10)).bold().cyan()));
            for (i, path) in progress.sample_paths.iter().take(10).enumerate() {
                text_content.push(Line::from(vec![
                    format!("  {}: ", i + 1).blue(),
                    path.clone().light_blue()
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
        *scroll_state = scroll_state.content_length(content_height as usize)
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

    fn render_visualization(self, progress: &AnalysisProgress, area: Rect, buffer: &mut Buffer) {
        if progress.entry_statuses.is_empty() {
            Paragraph::new("No data available yet. Processing...")
                .block(self.block())
                .render(area, buffer);
            return;
        }

        Canvas::default()
            .block(self.block().title("Entry Health Visualization"))
            .marker(Marker::Block)
            .x_bounds([0.0, 100.0])
            .y_bounds([0.0, 50.0])
            .paint(|context| {
                let canvas_width = 100.0;
                let canvas_height = 50.0;
                let total_pixels = (canvas_width * canvas_height) as usize;
                
                let entries_per_pixel = (progress.entry_statuses.len() as f64 / total_pixels as f64).ceil() as usize;
                let entries_per_pixel = entries_per_pixel.max(1);
                
                for pixel_index in 0..total_pixels {
                    let start_entry = pixel_index * entries_per_pixel;
                    let end_entry = ((pixel_index + 1) * entries_per_pixel).min(progress.entry_statuses.len());
                    
                    if start_entry >= progress.entry_statuses.len() {
                        break;
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
                    
                    let pixel_x = pixel_index % canvas_width as usize;
                    let pixel_y = pixel_index / canvas_width as usize;
                    
                    let total_entries_in_pixel = valid_count + invalid_count;
                    let color = if total_entries_in_pixel == 0 {
                        Color::Black
                    } else if invalid_count == 0 {
                        Color::Green
                    } else if valid_count == 0 {
                        Color::Red
                    } else {
                        let valid_ratio = valid_count as f64 / total_entries_in_pixel as f64;
                        if valid_ratio > 0.8 {
                            Color::Yellow
                        } else if valid_ratio > 0.5 {
                            Color::Rgb(255, 165, 0)
                        } else {
                            Color::Red
                        }
                    };
                    
                    context.draw(&Rectangle {
                        x: pixel_x as f64,
                        y: (canvas_height as usize - pixel_y - 1) as f64,
                        width: 1.0,
                        height: 1.0,
                        color,
                    });
                }
                
                if area.height >= 50 {
                    context.print(2.0, 45.0, "Legend:".white().bold());
                    context.print(2.0, 43.0, "■ Green = All Valid".green().bold());
                    context.print(2.0, 41.0, "■ Yellow = Mostly Valid".yellow().bold());
                    context.print(2.0, 39.0, "■ Orange = Mixed".fg(Color::Rgb(255, 165, 0)).bold());
                    context.print(2.0, 37.0, "■ Red = All Invalid".red().bold());
                    
                    context.print(2.0, 34.0, format!("Total: {}", progress.total_entries).white().bold());
                    context.print(2.0, 32.0, format!("Valid: {}", progress.valid_entries).green().bold());
                    context.print(2.0, 30.0, format!("Invalid: {}", progress.error_entries).red().bold());
                    context.print(2.0, 28.0, format!("Entries/pixel: {}", entries_per_pixel).cyan());
                } else {
                    context.print(2.0, 45.0, format!("Total: {} Valid: {} Invalid: {}", 
                        progress.total_entries, 
                        progress.valid_entries, 
                        progress.error_entries).white());
                }
            })
            .render(area, buffer);
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
            
            content.push(Line::from(format!("Progress: {}/{} entries ({:.1}%)", 
                progress.total_entries, estimated_total, progress_ratio * 100.0)));
        } else {
            content.push(Line::from(format!("Progress: {} entries processed", progress.total_entries)));
        }
        
        content.push(Line::from(""));
        
        // Performance metrics
        content.push(Line::from("Performance:"));
        content.push(Line::from(format!("  Processing rate: {:.0} entries/second", progress.entries_per_second)));
        
        let elapsed = if let Some(completion_time) = progress.completion_time {
            completion_time.duration_since(progress.start_time)
        } else {
            progress.start_time.elapsed()
        };
        content.push(Line::from(format!("  Elapsed time: {}", format_duration(elapsed))));
        
        if let Some(eta) = progress.eta_seconds {
            if eta > 0.0 && !progress.is_complete {
                let eta_duration = Duration::from_secs_f64(eta);
                content.push(Line::from(format!("  Estimated time remaining: {}", format_duration(eta_duration))));
            }
        }
        
        content.push(Line::from(""));
        
        // Statistics
        content.push(Line::from("Current Statistics:"));
        content.push(Line::from(format!("  Valid entries: {} ({:.1}%)", 
            progress.valid_entries, 
            if progress.total_entries > 0 { 
                progress.valid_entries as f64 / progress.total_entries as f64 * 100.0 
            } else { 0.0 })));
        content.push(Line::from(format!("  Error entries: {} ({:.1}%)", 
            progress.error_entries,
            if progress.total_entries > 0 { 
                progress.error_entries as f64 / progress.total_entries as f64 * 100.0 
            } else { 0.0 })));
        content.push(Line::from(format!("  Filename attributes found: {}", progress.filename_entries)));
        
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
