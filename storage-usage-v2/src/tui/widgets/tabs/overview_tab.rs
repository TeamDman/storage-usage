use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use humansize::DECIMAL;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Cell;
use ratatui::widgets::Row;
use ratatui::widgets::Table;
use ratatui::widgets::Widget;
use std::time::Duration;
use std::time::Instant;
use uom::ConstZero;
use uom::si::f64::Information;
use uom::si::f64::InformationRate;
use uom::si::f64::Time;
use uom::si::information::byte;
use uom::si::information_rate::byte_per_second;
use uom::si::ratio::ratio;
use uom::si::time::millisecond;
use uom::si::time::second;

pub struct OverviewTab;

impl OverviewTab {
    pub fn new() -> Self {
        Self
    }

    fn format_number(num: u64) -> String {
        let num_str = num.to_string();
        let mut result = String::new();
        let chars: Vec<char> = num_str.chars().collect();

        for (i, ch) in chars.iter().enumerate() {
            if i > 0 && (chars.len() - i) % 3 == 0 {
                result.push(',');
            }
            result.push(*ch);
        }
        result
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
        let rows: Vec<Row> = mft_files
            .iter()
            .map(|mft| {
                // Status column
                let status = if mft.processing_end.is_some() {
                    Text::from("OK").fg(Color::Green)
                } else {
                    Text::from("...").fg(Color::Yellow)
                };

                // File name column
                let file_name = match mft.path.file_name().and_then(|n| n.to_str()) {
                    Some(name) => name.to_string(),
                    None => format!("Unknown file {:?}", mft.path.to_string_lossy()),
                };

                // Progress column (with rate and remaining)
                let elapsed_time =
                    Time::new::<millisecond>(processing_begin.elapsed().as_millis() as f64);
                let progress_cell = if mft.processing_end.is_some() {
                    // When processing is complete, just show the processed size
                    Cell::from(humansize::format_size_i(mft.processed_size.get::<byte>(), DECIMAL))
                } else if elapsed_time > Time::ZERO {
                    let rate: InformationRate =
                        (mft.processed_size.get::<byte>() / elapsed_time).into();
                    let rate_text = format!(
                        " (+{}/s)",
                        humansize::format_size_i(rate.get::<byte_per_second>(), DECIMAL)
                    );

                    let base_text = format!(
                        "{}/{}",
                        humansize::format_size_i(mft.processed_size.get::<byte>(), DECIMAL),
                        match mft.total_size {
                            Some(total_size) =>
                                humansize::format_size_i(total_size.get::<byte>(), DECIMAL),
                            None => "? bytes".to_string(),
                        }
                    );

                    let mut spans = vec![
                        Span::raw(base_text),
                        Span::raw(rate_text).fg(Color::Cyan),
                    ];

                    if let Some(total_size) = mft.total_size {
                        let remaining = total_size - mft.processed_size;
                        let remaining_text = format!(" ({})", humansize::format_size_i(remaining.get::<byte>(), DECIMAL));
                        spans.push(Span::raw(remaining_text).fg(Color::Yellow));
                    }

                    Cell::from(Text::from(Line::from(spans)))
                } else {
                    Cell::from(format!(
                        "{}/{}",
                        humansize::format_size_i(mft.processed_size.get::<byte>(), DECIMAL),
                        match mft.total_size {
                            Some(total_size) =>
                                humansize::format_size_i(total_size.get::<byte>(), DECIMAL),
                            None => "? bytes".to_string(),
                        }
                    ))
                };

                // Entries column (with rate and remaining)
                let entries_cell = if let Some(entry_size) = mft.entry_size {
                    let processed_entries = if entry_size > Information::ZERO {
                        let x = mft.processed_size / entry_size;
                        x.get::<ratio>()
                    } else {
                        0.0
                    };

                    if mft.processing_end.is_some() {
                        // When processing is complete, just show the processed entries
                        Cell::from(Self::format_number(processed_entries as u64))
                    } else {
                        let total_entries = if let Some(total_size) = mft.total_size {
                            if entry_size > Information::ZERO {
                                Some(total_size.get::<byte>() / entry_size.get::<byte>())
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let base_text = match total_entries {
                            Some(total) => format!(
                                "{}/{}",
                                Self::format_number(processed_entries as u64),
                                Self::format_number(total as u64)
                            ),
                            None => format!("{}/?", Self::format_number(processed_entries as u64)),
                        };

                        if elapsed_time > Time::ZERO {
                            let entries_per_sec =
                                processed_entries as f64 / elapsed_time.get::<second>() as f64;
                            let rate_text =
                                format!(" (+{}/s)", Self::format_number(entries_per_sec as u64));

                            let mut spans =
                                vec![Span::raw(base_text), Span::raw(rate_text).fg(Color::Cyan)];

                            if let Some(total) = total_entries {
                                let remaining = total - processed_entries;
                                let remaining_text =
                                    format!(" ({})", Self::format_number(remaining as u64));
                                spans.push(Span::raw(remaining_text).fg(Color::Yellow));
                            }

                            Cell::from(Text::from(Line::from(spans)))
                        } else {
                            Cell::from(base_text)
                        }
                    }
                } else {
                    if mft.processing_end.is_some() {
                        Cell::from("?")
                    } else {
                        Cell::from("?/?")
                    }
                };

                // Time elapsed column
                let time_elapsed = humantime::format_duration(Duration::from_millis(
                    mft.processing_end
                        .map(|end| end.duration_since(processing_begin))
                        .unwrap_or_else(|| Instant::now().duration_since(processing_begin))
                        .as_millis() as u64,
                ))
                .to_string();

                // ETA column
                let eta = if mft.processing_end.is_none()
                    && let Some(total_size) = mft.total_size
                {
                    let elapsed =
                        Time::new::<millisecond>(processing_begin.elapsed().as_millis() as f64);
                    if elapsed == Time::ZERO {
                        "Processing...".to_string()
                    } else {
                        let remaining = total_size - mft.processed_size;
                        let rate: InformationRate =
                            (mft.processed_size.get::<byte>() / elapsed).into();
                        if rate == InformationRate::ZERO {
                            "Calculating rate...".to_string()
                        } else {
                            let estimated_remaining_duration = remaining / rate;
                            humantime::format_duration(Duration::from_secs(
                                estimated_remaining_duration.get::<second>() as u64,
                            ))
                            .to_string()
                        }
                    }
                } else {
                    "-".to_string()
                };

                // Add error information to ETA column if there are errors
                let eta_with_errors = if !mft.errors.is_empty() {
                    format!("{} (Errors: {})", eta, mft.errors.len())
                } else {
                    eta
                };

                Row::new(vec![
                    Cell::from(status),
                    Cell::from(file_name),
                    progress_cell,
                    entries_cell,
                    Cell::from(time_elapsed),
                    Cell::from(eta_with_errors),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(3), // Status
                Constraint::Min(16),   // File name
                Constraint::Fill(3),   // Progress (with rate)
                Constraint::Fill(3),   // Entries (with rate)
                Constraint::Fill(1),   // Time
                Constraint::Fill(1),   // ETA
            ],
        )
        .header(Row::new(vec![
            Cell::from(""),
            Cell::from("File"),
            Cell::from("Progress"),
            Cell::from("Entries"),
            Cell::from("Time"),
            Cell::from("ETA"),
        ]));

        table.render(area, buf);
    }
}
