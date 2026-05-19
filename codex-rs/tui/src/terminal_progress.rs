//! Ghostty terminal progress-bar output helpers for the TUI.
//!
//! Ghostty renders ConEmu-style OSC 9;4 progress reports as a thin graphical
//! bar at the top of the terminal surface. Codex only needs two states today:
//! an indeterminate "working" bar while an agent turn is active, and a clear
//! command once that work settles.

use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::io::stdout;

use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use crossterm::Command;
use ratatui::crossterm::execute;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TerminalProgressState {
    Remove,
    Indeterminate,
}

impl TerminalProgressState {
    fn osc_code(self) -> char {
        match self {
            Self::Remove => '0',
            Self::Indeterminate => '3',
        }
    }
}

/// Shows Ghostty's indeterminate graphical progress bar when supported.
pub(crate) fn show_terminal_progress_indeterminate() -> io::Result<()> {
    write_terminal_progress(TerminalProgressState::Indeterminate)
}

/// Clears Ghostty's graphical progress bar when supported.
pub(crate) fn clear_terminal_progress() -> io::Result<()> {
    write_terminal_progress(TerminalProgressState::Remove)
}

fn write_terminal_progress(state: TerminalProgressState) -> io::Result<()> {
    if !stdout().is_terminal() {
        return Ok(());
    }

    let terminal = terminal_info();
    if terminal.name != TerminalName::Ghostty {
        return Ok(());
    }

    execute!(
        stdout(),
        SetTerminalProgress {
            state,
            dcs_passthrough: matches!(terminal.multiplexer, Some(Multiplexer::Tmux { .. })),
        }
    )
}

#[derive(Debug, Clone, Copy)]
struct SetTerminalProgress {
    state: TerminalProgressState,
    dcs_passthrough: bool,
}

impl Command for SetTerminalProgress {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        let state = self.state.osc_code();
        if self.dcs_passthrough {
            write!(f, "\x1bPtmux;\x1b\x1b]9;4;{state}\x07\x1b\\")
        } else {
            write!(f, "\x1b]9;4;{state}\x07")
        }
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute SetTerminalProgress using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use crossterm::Command;
    use pretty_assertions::assert_eq;

    use super::SetTerminalProgress;
    use super::TerminalProgressState;

    #[test]
    fn indeterminate_progress_writes_plain_osc9_sequence() {
        let mut ansi = String::new();
        let command = SetTerminalProgress {
            state: TerminalProgressState::Indeterminate,
            dcs_passthrough: false,
        };

        command
            .write_ansi(&mut ansi)
            .expect("OSC 9;4 command should format");

        assert_eq!(ansi, "\u{1b}]9;4;3\u{7}");
    }

    #[test]
    fn clear_progress_writes_tmux_passthrough_sequence() {
        let mut ansi = String::new();
        let command = SetTerminalProgress {
            state: TerminalProgressState::Remove,
            dcs_passthrough: true,
        };

        command
            .write_ansi(&mut ansi)
            .expect("OSC 9;4 command should format");

        assert_eq!(ansi, "\u{1b}Ptmux;\u{1b}\u{1b}]9;4;0\u{7}\u{1b}\\");
    }
}
