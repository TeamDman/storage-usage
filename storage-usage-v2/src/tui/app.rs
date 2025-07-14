use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::app_tabs::AppTabs;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use crate::tui::worker::start_workers;
use ratatui::crossterm::event;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEventKind;
use ratatui::style::Color;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tachyonfx::Effect;
use tachyonfx::EffectRenderer;
use tachyonfx::Interpolation;
use tachyonfx::Motion;
use tachyonfx::Shader;
use tachyonfx::fx;
use uom::ConstZero;
use uom::si::f64::Information;

pub struct MftShowApp {
    pub mft_files: Vec<MftFileProgress>,
    pub processing_begin: Instant,
    pub tabs: AppTabs,
    pub startup_effect: Option<Effect>,
    pub quit_effect: Option<Effect>,
    pub last_frame_time: Instant,
    pub is_quitting: bool,
}

impl MftShowApp {
    pub fn new(mft_files: Vec<PathBuf>) -> Self {
        let mft_files = mft_files
            .into_iter()
            .map(|path| MftFileProgress {
                path,
                total_size: None,
                entry_size: None,
                processed_size: Information::ZERO,
                processing_end: None,
                files_within: Vec::new(),
                errors: Vec::new(),
                entry_health_statuses: Vec::new(),
            })
            .collect();

        // Create startup effect - sweep in with fade and gentle settle
        let startup_effect = Some(fx::sweep_in(
            Motion::LeftToRight,
            15,
            0,
            Color::Black,
            (1200, Interpolation::QuadOut),
        ));

        // Create quit effect - fade out with slide
        let quit_effect = Some(fx::sequence(&[fx::parallel(&[
            fx::fade_to_fg(Color::DarkGray, (800, Interpolation::SineIn)),
            fx::slide_out(
                Motion::RightToLeft,
                20,
                0,
                Color::Black,
                (1000, Interpolation::QuadIn),
            ),
        ])]));

        Self {
            mft_files,
            processing_begin: Instant::now(),
            tabs: AppTabs::new(),
            startup_effect,
            quit_effect,
            last_frame_time: Instant::now(),
            is_quitting: false,
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
        let mut handle = Some(handle);

        loop {
            // Calculate delta time for effects
            let now = Instant::now();
            let delta_time = now.duration_since(self.last_frame_time);
            self.last_frame_time = now;

            // Check if any effects are running
            let any_effect_running = self.startup_effect.as_ref().map_or(false, |e| e.running())
                || (self.is_quitting && self.quit_effect.as_ref().map_or(false, |e| e.running()));

            // Use shorter timeout when effects are running for smoother animation
            let poll_timeout = if any_effect_running {
                Duration::from_millis(1)
            } else {
                Duration::from_millis(10)
            };

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

                // Apply startup effect if it's running
                if let Some(ref mut effect) = self.startup_effect {
                    if effect.running() {
                        frame.render_effect(effect, frame.area(), delta_time.into());
                    } else {
                        // Effect is done, remove it to save resources
                        self.startup_effect = None;
                    }
                }

                // Apply quit effect if quitting
                if self.is_quitting {
                    if let Some(ref mut effect) = self.quit_effect {
                        frame.render_effect(effect, frame.area(), delta_time.into());

                        // If quit effect is done, break the loop
                        if !effect.running() {
                            return;
                        }
                    }
                }
            })?;

            // Break immediately if quit effect is done
            if self.is_quitting && self.quit_effect.as_ref().map_or(true, |e| !e.running()) {
                break;
            }

            if event::poll(poll_timeout)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    if !self.is_quitting {
                        self.is_quitting = true;
                        // Restart the quit effect
                        if let Some(ref mut effect) = self.quit_effect {
                            *effect = fx::sequence(&[fx::parallel(&[
                                fx::fade_to_fg(Color::DarkGray, (800, Interpolation::SineIn)),
                                fx::slide_out(
                                    Motion::RightToLeft,
                                    20,
                                    0,
                                    Color::Black,
                                    (1000, Interpolation::QuadIn),
                                ),
                            ])]);
                        }
                    }
                    handle.take();
                    continue; // Don't pass quit keys to tabs
                }

                // Pass key events to tabs only if not quitting
                if !self.is_quitting {
                    if let KeyboardResponse::Consume = self.tabs.on_key(key) {
                        // Key was handled by tabs
                    }
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
