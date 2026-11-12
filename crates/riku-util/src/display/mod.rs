//! User facing output in one clack style format, plus tables.
//!
//! Every command prints on the same rail (see [`rail`] for the layout).
//! Color is used only on a terminal and never when `NO_COLOR` is set; pipes
//! and `git push` get the same characters without escapes.
//!
//! | Function      | Line                  | Stream | Use for                          |
//! |---------------|-----------------------|--------|----------------------------------|
//! | `intro`       | `┌  title` then `│`    | stdout | first line of a multi step run   |
//! | `step_done`   | `◇  label  detail`    | stdout | a finished step                  |
//! | `info`        | `◇  msg`              | stdout | a step or status line            |
//! | `success`     | `◇  msg`              | stdout | a completed action               |
//! | `section`     | `◇  title`            | stdout | a heading over detail lines      |
//! | `warn`        | `▲  msg`              | stderr | non-fatal problems, hints        |
//! | `error`       | `■  msg`              | stderr | failures (then return `Err`)     |
//! | `step_failed` | `■  label  reason`    | stderr | the step that failed             |
//! | `emit`        | pre rendered text     | either | a block that must stay in order  |
//! | `note`        | `│  msg`              | stdout | detail under the step above      |
//! | `detail`      | `│  msg`              | stdout | detail under the step above      |
//! | `detail_err`  | `│  msg`              | stderr | detail that belongs with errors  |
//! | `kv`          | `│  key   value`      | stdout | key and value detail lines       |
//! | `blank`       | `│`                   | stdout | separator                        |
//! | `outro`       | `│` then `└  msg`     | stdout | last line of a multi step run    |
//! | `summary`     | aligned `key  value`  | stdout | facts to keep, after the outro   |
//! | `echo`        | by color name         | varies | older call sites                 |
//!
//! Tables (`format_table`, `print_table`) keep their column form.
//! Machine readable output (agent JSON, `logs`) never goes through here.

mod rail;
mod style;
mod table;
mod tail;

pub use rail::{
    detail, detail_err, emit, error, failure_already_shown, intro, mark_failure_shown, outro,
    render_line, render_summary, step_done, step_failed, summary, warn, Mark,
};
pub use style::{paint, Paint, Stream};
pub use table::{format_table, print_table, print_table_with_title};
pub use tail::{OutputTail, FAILURE_TAIL_LINES};

/// The prefix older deploy messages carry, Heroku style.
const ARROW: &str = "-----> ";
/// The indent older detail lines carry, the width of [`ARROW`].
const ARROW_INDENT: &str = "       ";

/// `◇  msg`: a step or status line.
pub fn info(msg: &str) {
    step_done(msg, "");
}

/// `◇  msg`: a completed action.
pub fn success(msg: &str) {
    step_done(msg, "");
}

/// `│  msg`: a detail line.
pub fn note(msg: &str) {
    detail(msg);
}

/// `◇  title`: a heading over the detail lines that follow.
pub fn section(title: &str) {
    step_done(title, "");
}

/// `│  key   value`: a detail line holding one key and value.
pub fn kv(key: &str, value: &str) {
    detail(&format!("{key:<20} {value}"));
}

/// A bare `│` separator line.
pub fn blank() {
    detail("");
}

/// Print `msg` by color name, for call sites written before the rail.
///
/// `green` is a step, `yellow` a warning and `red` an error; any other name
/// is a detail line. A message indented with spaces is a detail line under
/// the step above whatever its color, keeping any indent beyond the old
/// seven space one, and a leading `-----> ` is dropped.
pub fn echo(msg: &str, colour: &str) {
    match echo_mark(msg, colour) {
        (Mark::Step, text) => step_done(text, ""),
        (Mark::Warn, text) => warn(text),
        (Mark::Error, text) => error(text),
        (_, text) if colour == "yellow" || colour == "red" => detail_err(text),
        (_, text) => detail(text),
    }
}

/// The rail mark and text `echo` prints for `msg`.
fn echo_mark<'a>(msg: &'a str, colour: &str) -> (Mark, &'a str) {
    let text = msg.strip_prefix(ARROW).unwrap_or(msg);
    if text.is_empty() || text.starts_with(' ') {
        return (Mark::Rail, text.strip_prefix(ARROW_INDENT).unwrap_or(text));
    }
    let mark = match colour {
        "green" => Mark::Step,
        "yellow" => Mark::Warn,
        "red" => Mark::Error,
        _ => Mark::Rail,
    };
    (mark, text)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
