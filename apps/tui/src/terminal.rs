use std::io::{IsTerminal, Stdout, stdout};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    pub fn enter() -> Result<Self, TerminalError> {
        let mut output = stdout();
        if !output.is_terminal() {
            return Err(TerminalError::NotATerminal);
        }
        enable_raw_mode()?;
        if let Err(error) = execute!(
            output,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
            Hide
        ) {
            let _ = disable_raw_mode();
            return Err(TerminalError::Io(error));
        }
        let terminal = Terminal::new(CrosstermBackend::new(output))?;
        Ok(Self { terminal })
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = execute!(
            self.terminal.backend_mut(),
            PopKeyboardEnhancementFlags,
            DisableBracketedPaste,
            DisableMouseCapture,
            Show,
            LeaveAlternateScreen
        );
        let _ = disable_raw_mode();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("luna-tui must run in a terminal; use ssh -t when connecting remotely")]
    NotATerminal,
    #[error("terminal setup failed: {0}")]
    Io(#[from] std::io::Error),
}
