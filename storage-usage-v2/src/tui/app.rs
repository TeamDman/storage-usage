use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::app_tabs::AppTabs;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use crate::tui::worker::start_workers;
use ratatui::crossterm::event;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEventKind;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use uom::si::information::byte;
use uom::si::u64::Information;

pub struct MftShowApp {
    pub mft_files: Vec<MftFileProgress>,
    pub processing_begin: Instant,
    pub tabs: AppTabs,
}

impl MftShowApp {
    pub fn new(mft_files: Vec<PathBuf>) -> Self {
        let mft_files = mft_files
            .into_iter()
            .map(|path| MftFileProgress {
                path,
                total_size: None,
                processed_size: Information::new::<byte>(0),
                processing_end: None,
                files_within: Vec::new(),
                errors: Vec::new(),
                entry_health_statuses: Vec::new(),
            })
            .collect();
        Self {
            mft_files,
            processing_begin: Instant::now(),
            tabs: AppTabs::new(),
        }
    }
    pub fn run(mut self) -> eyre::Result<()> {
        let (rx, handle) = start_workers(
            self.mft_files
                .iter()
                .map(|progress| progress.path.clone())
                .collect(),
        )?;

        let mut terminal = ratatui::init();
        terminal.clear()?;
        const POLL_TIMEOUT: Duration = Duration::from_millis(100);
        let mut handle = Some(handle);
        loop {
            // process messages
            while let Ok(message) = rx.try_recv() {
                message.handle(&mut self.mft_files)?;
            }
            terminal.draw(|frame| {
                self.tabs.render(
                    frame.area(),
                    frame.buffer_mut(),
                    &self.mft_files,
                    self.processing_begin,
                );
            })?;
            if event::poll(POLL_TIMEOUT)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    handle.take();
                    break;
                }

                // Pass key events to tabs

                if let KeyboardResponse::Consume = self.tabs.on_key(key) {
                    // Key was handled by tabs
                }
            }
        }

        ratatui::restore();
        if let Some(handle) = handle.take() {
            handle
                .join()
                .map_err(|_| eyre::eyre!("Worker thread panicked"))??;
        }
        Ok(())
    }
}
