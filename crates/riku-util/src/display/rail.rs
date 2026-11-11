//! The clack style rail: an intro, one line per finished step, warnings,
//! errors and detail lines on a vertical rail, an outro, and an aligned
//! summary block after it.
//!
//! ```text
//! ┌  Deploying shop  54ba34f "Fix login"
//! │
//! ◇  Runtime  python
//! ▲  ENV holds riku settings
//! │  detail under the step above
//! │
//! └  Deployed in 18.2s
//!
//!    App       shop
//!    Logs      rk logs shop
//! ```
//!
//! The `render_*` functions build the text and take `colour` explicitly, so
//! tests see exactly what a pipe receives. The printing functions pick color
//! per stream (see [`super::style`]); errors and warnings go to stderr,
//! everything else to stdout.

use super::style::{paint, Paint, Stream};

/// What a rail line marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// `┌` the first line of a command's output.
    Intro,
    /// `◇` a finished step.
    Step,
    /// `▲` a non-fatal problem.
    Warn,
    /// `■` a failure.
    Error,
    /// `│` a detail line under the step above.
    Rail,
    /// `└` the last line before the summary.
    Outro,
}

impl Mark {
    fn glyph(self) -> &'static str {
        match self {
            Mark::Intro => "┌",
            Mark::Step => "◇",
            Mark::Warn => "▲",
            Mark::Error => "■",
            Mark::Rail => "│",
            Mark::Outro => "└",
        }
    }

    fn paint(self) -> Paint {
        match self {
            Mark::Step => Paint::Green,
            Mark::Warn => Paint::Yellow,
            Mark::Error => Paint::Red,
            Mark::Intro | Mark::Rail | Mark::Outro => Paint::Gray,
        }
    }

    fn stream(self) -> Stream {
        match self {
            Mark::Warn | Mark::Error => Stream::Stderr,
            _ => Stream::Stdout,
        }
    }
}

/// One rail line, `label` then a dimmed `detail` when there is one. A label
/// spanning several lines continues on `│` lines, and an empty label gives
/// a bare `│`.
pub fn render_line(mark: Mark, label: &str, detail: &str, colour: bool) -> String {
    let glyph = paint(mark.glyph(), mark.paint(), colour);
    let mut lines = label.lines();
    let first = lines.next().unwrap_or("");
    let mut out = if first.is_empty() && detail.is_empty() {
        glyph
    } else {
        format!("{glyph}  {}", styled_label(mark, first, colour))
    };
    if !detail.is_empty() {
        out.push_str("  ");
        out.push_str(&paint(detail, Paint::Dim, colour));
    }
    let rail = paint(Mark::Rail.glyph(), Mark::Rail.paint(), colour);
    for rest in lines {
        out.push('\n');
        out.push_str(&rail);
        if !rest.is_empty() {
            out.push_str("  ");
            out.push_str(rest);
        }
    }
    out
}

fn styled_label(mark: Mark, label: &str, colour: bool) -> String {
    match mark {
        Mark::Error => paint(label, Paint::Red, colour),
        Mark::Intro | Mark::Outro => paint(label, Paint::Bold, colour),
        _ => label.to_string(),
    }
}

/// The summary block printed after the outro: a blank line, then each key
/// and value with the values aligned.
pub fn render_summary(rows: &[(&str, &str)], colour: bool) -> String {
    let width = rows
        .iter()
        .map(|(key, _)| key.chars().count())
        .max()
        .unwrap_or(0)
        + 3;
    let mut out = String::new();
    for (key, value) in rows {
        let pad = " ".repeat(width - key.chars().count());
        out.push_str(&format!(
            "\n   {}{pad}{value}",
            paint(key, Paint::Bold, colour)
        ));
    }
    out
}

fn print(mark: Mark, label: &str, detail: &str) {
    let stream = mark.stream();
    let line = render_line(mark, label, detail, stream.colour());
    match stream {
        Stream::Stdout => println!("{line}"),
        Stream::Stderr => eprintln!("{line}"),
    }
}

/// `┌  title` followed by a bare rail line.
pub fn intro(title: &str) {
    print(Mark::Intro, title, "");
    print(Mark::Rail, "", "");
}

/// `◇  label  detail`: a finished step.
pub fn step_done(label: &str, detail: &str) {
    print(Mark::Step, label, detail);
}

/// `▲  msg` on stderr: a non-fatal problem.
pub fn warn(msg: &str) {
    print(Mark::Warn, msg, "");
}

/// `■  msg` in red on stderr: a failure.
pub fn error(msg: &str) {
    print(Mark::Error, msg, "");
}

/// `■  label  detail` in red on stderr: a step that failed, with its reason.
pub fn step_failed(label: &str, reason: &str) {
    print(Mark::Error, label, reason);
}

/// `│  msg`: a detail line under the step above. An empty `msg` prints a
/// bare rail.
pub fn detail(msg: &str) {
    print(Mark::Rail, msg, "");
}

/// `│  msg` on stderr: a detail line that belongs with errors, such as a
/// streamed command's stderr.
pub fn detail_err(msg: &str) {
    eprintln!(
        "{}",
        render_line(Mark::Rail, msg, "", Stream::Stderr.colour())
    );
}

/// A bare rail line, then `└  msg`.
pub fn outro(msg: &str) {
    print(Mark::Rail, "", "");
    print(Mark::Outro, msg, "");
}

/// The aligned key and value block after [`outro`].
pub fn summary(rows: &[(&str, &str)]) {
    if rows.is_empty() {
        return;
    }
    println!("{}", render_summary(rows, Stream::Stdout.colour()));
}

#[cfg(test)]
#[path = "rail_tests.rs"]
mod tests;
