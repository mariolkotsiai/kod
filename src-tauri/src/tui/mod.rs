pub mod views;

use crate::core::device_pool::DevicePool;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use views::dashboard::render_dashboard;

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("Terminal I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Runs the standalone terminal UI (`--tui`) until user presses `q` or `Esc`.
pub async fn run_tui(pool: Arc<DevicePool>) -> Result<(), TuiError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let run_result = async {
        loop {
            let slots = pool.list_slots().await;
            terminal.draw(|frame| render_dashboard(frame, &slots))?;

            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }
        Ok::<(), TuiError>(())
    }
    .await;

    // Always restore terminal state even if loop exits early or encounters an error.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    run_result
}