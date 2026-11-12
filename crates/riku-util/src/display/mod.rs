//! User facing output in one clack style format, plus tables.
//!
//! Every command prints on the same rail (see [`rail`] for the layout).
//! Color is used only on a terminal and never when `NO_COLOR` is set; pipes
//! and `git push` get the same characters without escapes.
//!
//! | Function      | Line                  | Stream | Use for                          |
//! |---------------|-----------------------|--------|----------------------------------|
//! | `intro`       | `┌  title` then `│`    | stdout | first line of a multi step run   |
//! | `step_done`   | `◇  label  detail`    | stdout | a finished step or status line   |
//! | `warn`        | `▲  msg`              | stderr | non-fatal problems, hints        |
//! | `error`       | `■  msg`              | stderr | failures (then return `Err`)     |
//! | `step_failed` | `■  label  reason`    | stderr | the step that failed             |
//! | `emit`        | pre rendered text     | either | a block that must stay in order  |
//! | `detail`      | `│  msg`              | stdout | detail under the step above      |
//! | `detail_err`  | `│  msg`              | stderr | detail that belongs with errors  |
//! | `outro`       | `│` then `└  msg`     | stdout | last line of a multi step run    |
//! | `summary`     | aligned `key  value`  | stdout | facts to keep, after the outro   |
//!
//! `detail("")` prints a bare `│` separator, and a message's leading spaces
//! are kept after the `│  `.
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
