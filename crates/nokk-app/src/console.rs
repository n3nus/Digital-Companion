use std::io::{Write, stdout};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use nokk_core::{AppConfig, Bounds, PetBrain, SpriteSheet};

use crate::app_assets;

pub fn run() -> Result<()> {
    let sheet = app_assets::load_sprites()?;
    let config = AppConfig::load_or_default().unwrap_or_default();
    let mut brain = PetBrain::from_config(
        nokk_core::pet::unix_time_seed(),
        config.position,
        config.last_pose,
        config.mood,
        config.paused,
    );

    let mut terminal = TerminalSession::enter()?;
    let started = Instant::now();

    loop {
        while event::poll(Duration::from_millis(0)).context("poll terminal input")? {
            if let Event::Key(key) = event::read().context("read terminal input")? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        let snapshot = brain.snapshot();
                        let saved = AppConfig {
                            position: config.position,
                            last_pose: snapshot.animation,
                            mood: snapshot.mood,
                            paused: snapshot.paused,
                            ..config
                        };
                        let _ = saved.save();
                        return Ok(());
                    }
                    KeyCode::Char('p') => brain.toggle_paused(),
                    _ => {}
                }
            }
        }

        let now_ms = started.elapsed().as_millis() as u64;
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let bounds = Bounds {
            width: i32::from(cols).max(20) * 8,
            height: i32::from(rows).max(10) * 16,
            pet_size: sheet.frame_size() as i32,
        };
        brain.tick(now_ms, bounds);
        draw_console(&mut terminal.stdout, &sheet, &brain, now_ms)?;
        std::thread::sleep(Duration::from_millis(80));
    }
}

fn draw_console(
    stdout: &mut std::io::Stdout,
    sheet: &SpriteSheet,
    brain: &PetBrain,
    now_ms: u64,
) -> Result<()> {
    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    let snapshot = brain.snapshot();
    let frame = brain.current_frame(sheet.manifest(), now_ms);
    let (terminal_cols, terminal_rows) = terminal::size().unwrap_or((80, 24));
    let available_cols = u32::from(terminal_cols.saturating_sub(2)).max(1);
    let available_pixel_rows = u32::from(terminal_rows.saturating_sub(5)).max(1) * 2;
    let target_pixels = sheet
        .frame_size()
        .min(64)
        .min(available_cols)
        .min(available_pixel_rows)
        .max(12);
    let sample = sheet.frame_size().div_ceil(target_pixels).max(1);
    let output_pixels = (sheet.frame_size() / sample).max(1);
    let left_pad = ((u32::from(terminal_cols).saturating_sub(output_pixels)) / 2).min(8);

    queue!(
        stdout,
        SetForegroundColor(Color::DarkGreen),
        Print("Nøkk "),
        ResetColor,
        Print(if snapshot.paused {
            "paused"
        } else {
            "wandering"
        }),
        Print("\r\n\r\n")
    )?;

    for y in (0..output_pixels).step_by(2) {
        if left_pad > 0 {
            queue!(stdout, Print(" ".repeat(left_pad as usize)))?;
        }

        for x in 0..output_pixels {
            let upper = sample_color(sheet, frame, x * sample, y * sample, sample);
            let lower = if y + 1 < output_pixels {
                sample_color(sheet, frame, x * sample, (y + 1) * sample, sample)
            } else {
                None
            };

            queue!(stdout, ResetColor)?;
            match (upper, lower) {
                (Some(foreground), Some(background)) => queue!(
                    stdout,
                    SetForegroundColor(foreground),
                    SetBackgroundColor(background),
                    Print("▀")
                )?,
                (Some(foreground), None) => {
                    queue!(stdout, SetForegroundColor(foreground), Print("▀"))?
                }
                (None, Some(foreground)) => {
                    queue!(stdout, SetForegroundColor(foreground), Print("▄"))?
                }
                (None, None) => queue!(stdout, Print(" "))?,
            }
        }
        queue!(stdout, ResetColor, Print("\r\n"))?;
    }

    queue!(stdout, ResetColor, Print("\r\nq quit  p pause\r\n"))?;
    stdout.flush()?;
    Ok(())
}

fn sample_color(
    sheet: &SpriteSheet,
    frame: u16,
    x_start: u32,
    y_start: u32,
    sample: u32,
) -> Option<Color> {
    let x_end = (x_start + sample).min(sheet.frame_size());
    let y_end = (y_start + sample).min(sheet.frame_size());
    if x_start >= x_end || y_start >= y_end {
        return None;
    }

    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;
    let mut alpha = 0u32;
    let mut samples = 0u32;

    for y in y_start..y_end {
        for x in x_start..x_end {
            let px = sheet.frame_pixel(frame, x, y);
            samples += 1;
            if px.a > 8 {
                let weight = u32::from(px.a);
                r += u32::from(px.r) * weight;
                g += u32::from(px.g) * weight;
                b += u32::from(px.b) * weight;
                alpha += weight;
            }
        }
    }

    if samples == 0 || alpha / samples < 18 {
        return None;
    }

    Some(Color::Rgb {
        r: (r / alpha) as u8,
        g: (g / alpha) as u8,
        b: (b / alpha) as u8,
    })
}

struct TerminalSession {
    stdout: std::io::Stdout,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        let mut stdout = stdout();
        terminal::enable_raw_mode().context("enable terminal raw mode")?;
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { stdout })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen, ResetColor);
        let _ = terminal::disable_raw_mode();
    }
}
