use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use humansize::DECIMAL;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::List;
use ratatui::widgets::Widget;
use uom::si::u64::InformationRate;
use std::time::Duration;
use std::time::Instant;
use uom::si::information::byte;
use uom::si::information_rate::byte_per_second;
use uom::si::time::millisecond;
use uom::si::time::nanosecond;
use uom::si::time::second;
use uom::si::u64::Time;

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
        let mut lines = Vec::new();
        for mft in mft_files {
            let mut text = Text::default();

            if mft.processing_end.is_some() {
                text.push_span(Span::raw("OK ").fg(Color::Green));
            } else {
                text.push_span(Span::raw("...").fg(Color::Yellow));
            }
            text.push_span(Span::raw(" "));

            text.push_span({
                match mft.path.file_name().and_then(|n| n.to_str()) {
                    Some(name) => Span::raw(name),
                    None => Span::raw(format!("Unknown file {:?}", mft.path.to_string_lossy())),
                }
            });
            text.push_span(Span::raw(" "));

            text.push_span(Span::from(humansize::format_size(
                mft.processed_size.get::<byte>(),
                DECIMAL,
            )));
            text.push_span(Span::raw("/"));
            text.push_span(match mft.total_size {
                Some(total_size) => {
                    Span::from(humansize::format_size(total_size.get::<byte>(), DECIMAL))
                }
                None => Span::raw("? bytes"),
            });
            text.push_span(Span::raw(" "));

            text.push_span(Span::raw(
                humantime::format_duration(
                    mft.processing_end
                        .map(|end| end.duration_since(processing_begin))
                        .unwrap_or_else(|| Instant::now().duration_since(processing_begin)),
                )
                .to_string(),
            ));
            text.push_span(Span::raw(" "));

            if mft.processing_end.is_none()
                && let Some(total_size) = mft.total_size
            {
                let elapsed =
                    Time::new::<millisecond>(processing_begin.elapsed().as_millis() as u64);
                if elapsed.get::<nanosecond>() == 0 {
                    text.push_span(Span::raw(" (Processing...)"));
                } else {
                    let remaining = total_size - mft.processed_size;
                    let rate: InformationRate = (mft.processed_size.get::<byte>() / elapsed).into();
                    if rate.get::<byte_per_second>() == 0 {
                        text.push_span(Span::raw(" (Calculating rate...)"));
                    } else {
                        let estimated_remaining_duration = remaining / rate;
                        let estimated_remaining_duration = humantime::format_duration(
                            Duration::from_secs(estimated_remaining_duration.get::<second>() as u64),
                        );
                        text.push_span(Span::raw(" (ETA: "));
                        text.push_span(Span::raw(estimated_remaining_duration.to_string()));
                        text.push_span(Span::raw(")"));
                    }
                }
            }

            if !mft.errors.is_empty() {
                text.push_span(Span::raw(" (Errors: "));
                text.push_span(Span::raw(mft.errors.len().to_string()));
                text.push_span(Span::raw(")"));
            }

            lines.push(text);
        }
        List::new(lines).render(area, buf);
    }
}
