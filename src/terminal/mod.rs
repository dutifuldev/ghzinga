use std::io::{self, Stdout};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalGuard {
    mouse_enabled: bool,
}

impl TerminalGuard {
    pub fn enter(mouse_enabled: bool) -> anyhow::Result<(Self, AppTerminal)> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        if mouse_enabled {
            execute!(stdout, EnableMouseCapture)?;
        }
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok((Self { mouse_enabled }, terminal))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        if self.mouse_enabled {
            let _ = execute!(stdout, DisableMouseCapture);
        }
        let _ = execute!(stdout, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
