use std::io::{self, Stdout};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalGuard {
    raw_enabled: bool,
    alternate_screen: bool,
    mouse_enabled: bool,
}

impl TerminalGuard {
    pub fn enter(mouse_enabled: bool) -> anyhow::Result<(Self, AppTerminal)> {
        enable_raw_mode()?;
        let mut guard = Self {
            raw_enabled: true,
            alternate_screen: false,
            mouse_enabled: false,
        };
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        guard.alternate_screen = true;
        if mouse_enabled {
            execute!(stdout, EnableMouseCapture)?;
            guard.mouse_enabled = true;
        }
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok((guard, terminal))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        if self.mouse_enabled {
            let _ = execute!(stdout, DisableMouseCapture);
        }
        if self.alternate_screen {
            let _ = execute!(stdout, LeaveAlternateScreen);
        }
        if self.raw_enabled {
            let _ = disable_raw_mode();
        }
    }
}
