use humansize::DECIMAL;
use humansize::format_size;
use ratatui::prelude::*;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Gauge;
use ratatui::widgets::Widget;
use ratatui::text::{Line, Span};
use uom::si::f64::Information;
use uom::si::information::byte;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::Storage::FileSystem::GetVolumeInformationW;
use windows::core::PCWSTR;
use std::time::{Duration, Instant};

const MAX_PATH: usize = 260;

#[derive(Clone)]
struct DriveInfo {
    letter: String,
    label: String,
    total: Information,
    free: Information,
}

fn get_drive_info(drive_letter: &str) -> eyre::Result<DriveInfo> {
    let mut free_bytes = 0;
    let mut total_bytes = 0;
    let mut total_free_bytes = 0;
    let drive = format!("{}:\\", drive_letter);
    let drive_wide: Vec<u16> = drive.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let result = GetDiskFreeSpaceExW(
            PCWSTR(drive_wide.as_ptr()),
            Some(&mut free_bytes),
            Some(&mut total_bytes),
            Some(&mut total_free_bytes),
        );
        if result.is_err() {
            return Err(eyre::eyre!("Failed to get disk space for {drive_letter}"));
        }
    }
    // Get label
    let mut volume_name = [0u16; MAX_PATH + 1];
    unsafe {
        let _ = GetVolumeInformationW(
            PCWSTR(drive_wide.as_ptr()),
            Some(&mut volume_name),
            None,
            None,
            None,
            None,
        );
    }
    let label = {
        let raw = String::from_utf16_lossy(
            &volume_name[..volume_name.iter().position(|&c| c == 0).unwrap_or(0)],
        );
        if raw.trim().is_empty() {
            "Local Disk".to_string()
        } else {
            raw
        }
    };
    Ok(DriveInfo {
        letter: drive_letter.to_string(),
        label,
        total: Information::new::<byte>(total_bytes as f64),
        free: Information::new::<byte>(free_bytes as f64),
    })
}

fn get_all_drives() -> eyre::Result<Vec<DriveInfo>> {
    let drive_letters = ["C", "D", "E", "F", "G", "H", "I"];
    drive_letters.iter().map(|&d| get_drive_info(d)).collect()
}

fn gauge_color(free: &Information, total: &Information) -> Color {
    let free_bytes = free.get::<byte>();
    let total_bytes = total.get::<byte>();
    let percent_free = free_bytes / total_bytes.max(1.0);
    if free_bytes < 100.0 * 1024.0 * 1024.0 * 1024.0 || percent_free < 0.10 {
        Color::Red
    } else {
        Color::Blue
    }
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let mut drive_snapshots: Vec<Vec<DriveInfo>> = Vec::new();
    let mut drives = get_all_drives()?;
    drive_snapshots.push(drives.clone());
    let mut terminal = ratatui::init();
    let mut last_refresh = Instant::now();
    loop {
        // Refresh every 1 second
        if last_refresh.elapsed() >= Duration::from_secs(1) {
            drives = get_all_drives()?;
            drive_snapshots.push(drives.clone());
            last_refresh = Instant::now();
        }
        let total_space = drives.iter().map(|d| d.total).sum::<Information>();
        let total_free = drives.iter().map(|d| d.free).sum::<Information>();
        let total_used = total_space - total_free;
        terminal.draw(|frame| {
            let area = frame.area();
            let mut constraints = vec![Constraint::Length(3); drives.len()];
            constraints.push(Constraint::Length(3)); // for total gauge
            let num_rows = drives.len() + 1;
            let rows = match num_rows {
                1 => Layout::vertical(constraints).areas::<1>(area).to_vec(),
                2 => Layout::vertical(constraints).areas::<2>(area).to_vec(),
                3 => Layout::vertical(constraints).areas::<3>(area).to_vec(),
                4 => Layout::vertical(constraints).areas::<4>(area).to_vec(),
                5 => Layout::vertical(constraints).areas::<5>(area).to_vec(),
                6 => Layout::vertical(constraints).areas::<6>(area).to_vec(),
                7 => Layout::vertical(constraints).areas::<7>(area).to_vec(),
                8 => Layout::vertical(constraints).areas::<8>(area).to_vec(),
                _ => panic!("Too many drives for this layout!"),
            };
            // Per-drive gauges
            for (i, drive) in drives.iter().enumerate() {
                let used = drive.total - drive.free;
                let ratio = used.get::<byte>() / drive.total.get::<byte>().max(1.0);
                // Delta calculation
                let delta_span = if let (Some(first), Some(last)) = (
                    drive_snapshots.first().and_then(|snap| snap.get(i)),
                    drive_snapshots.last().and_then(|snap| snap.get(i)),
                ) {
                    let delta = last.free.get::<byte>() - first.free.get::<byte>();
                    if delta == 0.0 {
                        Span::raw("")
                    } else {
                        let (sign, color, abs_delta) = if delta < 0.0 {
                            ("-", Color::Red, -delta)
                        } else {
                            ("+", Color::Green, delta)
                        };
                        let human = format_size(abs_delta as u64, DECIMAL);
                        Span::styled(format!(" ({} {})", sign, human), Style::default().fg(color))
                    }
                } else {
                    Span::raw("")
                };
                let label = Line::from(vec![
                    Span::raw(format!(
                        "{} [{}]: {} / {}",
                        drive.letter,
                        drive.label,
                        format_size(used.get::<byte>() as u64, DECIMAL),
                        format_size(drive.total.get::<byte>() as u64, DECIMAL)
                    )),
                    Span::styled(
                        format!(" ({} free)", format_size(drive.free.get::<byte>() as u64, DECIMAL)),
                        Style::default().fg(Color::Magenta),
                    ),
                    delta_span,
                ]);
                Gauge::default()
                    .block(Block::default().title(label).borders(Borders::ALL))
                    .gauge_style(Style::default().fg(gauge_color(&drive.free, &drive.total)))
                    .ratio(ratio)
                    .render(rows[i], frame.buffer_mut());
            }
            // Total gauge
            let total_label = format!(
                "Total: {} / {}",
                format_size(total_used.get::<byte>() as u64, DECIMAL),
                format_size(total_space.get::<byte>() as u64, DECIMAL)
            );
            Gauge::default()
                .block(Block::default().title(total_label).borders(Borders::ALL))
                .gauge_style(Style::default().fg(gauge_color(&total_free, &total_space)))
                .ratio(total_used.get::<byte>() / total_space.get::<byte>().max(1.0))
                .render(rows[drives.len()], frame.buffer_mut());
        })?;
        // Keyboard event handler
        use ratatui::crossterm::event::{self, Event, KeyCode};
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        break;
                    }
                    KeyCode::Char('r') => {
                        drives = get_all_drives()?;
                        drive_snapshots.push(drives.clone());
                        last_refresh = Instant::now();
                    }
                    _ => {}
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::information::gigabyte;
    use uom::si::information::terabyte;
    #[test]
    fn test_gauge_color() {
        let total = Information::new::<terabyte>(1.0);
        let free = Information::new::<gigabyte>(200.0);
        assert_eq!(gauge_color(&free, &total), Color::Blue);
        let free = Information::new::<gigabyte>(50.0);
        assert_eq!(gauge_color(&free, &total), Color::Red);
        let free = Information::new::<gigabyte>(100.0);
        assert_eq!(gauge_color(&free, &total), Color::Red);
        let free = Information::new::<gigabyte>(150.0);
        assert_eq!(gauge_color(&free, &total), Color::Blue);
        let free = Information::new::<gigabyte>(90.0);
        assert_eq!(gauge_color(&free, &total), Color::Red);
    }
    #[test]
    fn test_drive_info_count_and_red_gauges() {
        let drives = get_all_drives().expect("Failed to get drive info");
        assert_eq!(drives.len(), 7, "Expected 7 drives");
        let red_count = drives.iter().filter(|d| gauge_color(&d.free, &d.total) == Color::Red).count();
        assert_eq!(red_count, 4, "Expected 4 drives to be red (low space)");
    }
}
