use std::{
    io::{self, Stdout},
    panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        Once,
    },
};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

static PANIC_HOOK: Once = Once::new();
static TERMINAL_STATE: TerminalState = TerminalState::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalSnapshot {
    raw_enabled: bool,
    alternate_screen: bool,
    mouse_enabled: bool,
}

struct TerminalState {
    raw_enabled: AtomicBool,
    alternate_screen: AtomicBool,
    mouse_enabled: AtomicBool,
}

impl TerminalState {
    const fn new() -> Self {
        Self {
            raw_enabled: AtomicBool::new(false),
            alternate_screen: AtomicBool::new(false),
            mouse_enabled: AtomicBool::new(false),
        }
    }

    fn set_raw_enabled(&self, enabled: bool) {
        self.raw_enabled.store(enabled, Ordering::SeqCst);
    }

    fn set_alternate_screen(&self, enabled: bool) {
        self.alternate_screen.store(enabled, Ordering::SeqCst);
    }

    fn set_mouse_enabled(&self, enabled: bool) {
        self.mouse_enabled.store(enabled, Ordering::SeqCst);
    }

    fn snapshot_and_clear(&self) -> TerminalSnapshot {
        TerminalSnapshot {
            raw_enabled: self.raw_enabled.swap(false, Ordering::SeqCst),
            alternate_screen: self.alternate_screen.swap(false, Ordering::SeqCst),
            mouse_enabled: self.mouse_enabled.swap(false, Ordering::SeqCst),
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> TerminalSnapshot {
        TerminalSnapshot {
            raw_enabled: self.raw_enabled.load(Ordering::SeqCst),
            alternate_screen: self.alternate_screen.load(Ordering::SeqCst),
            mouse_enabled: self.mouse_enabled.load(Ordering::SeqCst),
        }
    }
}

pub struct TerminalGuard;

impl TerminalGuard {
    pub fn enter(mouse_enabled: bool) -> anyhow::Result<(Self, AppTerminal)> {
        install_panic_hook();
        enable_raw_mode()?;
        let guard = Self;
        TERMINAL_STATE.set_raw_enabled(true);
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        TERMINAL_STATE.set_alternate_screen(true);
        if mouse_enabled {
            execute!(stdout, EnableMouseCapture)?;
            TERMINAL_STATE.set_mouse_enabled(true);
        }
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok((guard, terminal))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal_state();
    }
}

fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            restore_terminal_state();
            default_hook(info);
        }));
    });
}

fn restore_terminal_state() {
    let snapshot = TERMINAL_STATE.snapshot_and_clear();
    restore_snapshot(snapshot);
}

fn restore_snapshot(snapshot: TerminalSnapshot) {
    let mut stdout = io::stdout();
    if snapshot.mouse_enabled {
        let _ = execute!(stdout, DisableMouseCapture);
    }
    if snapshot.alternate_screen {
        let _ = execute!(stdout, LeaveAlternateScreen);
    }
    if snapshot.raw_enabled {
        let _ = disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_state_snapshot_and_clear_is_idempotent() {
        let state = TerminalState::new();
        state.set_raw_enabled(true);
        state.set_alternate_screen(true);
        state.set_mouse_enabled(true);

        assert_eq!(
            state.snapshot_and_clear(),
            TerminalSnapshot {
                raw_enabled: true,
                alternate_screen: true,
                mouse_enabled: true,
            }
        );
        assert_eq!(
            state.snapshot_and_clear(),
            TerminalSnapshot {
                raw_enabled: false,
                alternate_screen: false,
                mouse_enabled: false,
            }
        );
    }

    #[test]
    fn terminal_state_tracks_features_independently() {
        let state = TerminalState::new();
        state.set_raw_enabled(true);
        state.set_mouse_enabled(true);

        assert_eq!(
            state.snapshot(),
            TerminalSnapshot {
                raw_enabled: true,
                alternate_screen: false,
                mouse_enabled: true,
            }
        );
    }
}
