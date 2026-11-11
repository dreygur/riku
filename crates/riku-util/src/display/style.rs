//! When output gets color, and the few ANSI styles the rail uses.
//!
//! Color is on only when the stream a line goes to is a terminal and
//! `NO_COLOR` is unset or empty. Pipes, log files, CI and `git push` (whose
//! hook output reaches the pusher as `remote:` lines) get plain text with the
//! same rail characters.

use std::io::IsTerminal;

/// The stream a line is written to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    /// Whether lines written to this stream should carry color.
    pub fn colour(self) -> bool {
        let terminal = match self {
            Stream::Stdout => std::io::stdout().is_terminal(),
            Stream::Stderr => std::io::stderr().is_terminal(),
        };
        terminal && !no_colour_requested()
    }
}

fn no_colour_requested() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

/// An ANSI style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    Green,
    Yellow,
    Red,
    Cyan,
    Gray,
    Bold,
    Dim,
}

impl Paint {
    fn code(self) -> &'static str {
        match self {
            Paint::Green => "32",
            Paint::Yellow => "33",
            Paint::Red => "31",
            Paint::Cyan => "36",
            Paint::Gray => "90",
            Paint::Bold => "1",
            Paint::Dim => "2",
        }
    }
}

/// `text` in `style` when `colour` is on, unchanged otherwise.
pub fn paint(text: &str, style: Paint, colour: bool) -> String {
    if !colour || text.is_empty() {
        return text.to_string();
    }
    format!("\x1b[{}m{text}\x1b[0m", style.code())
}
