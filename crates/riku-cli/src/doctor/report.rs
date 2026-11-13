//! Printing the doctor report: one rail line per check, then the tally.

use crate::util::display::{self, Mark};

use super::{Check, Status};

/// The rail line of one check, its detail on the `│` lines under it. All on
/// stdout, so a piped report keeps its order.
fn render_check(check: &Check, colour: bool) -> String {
    let mark = match check.status {
        Status::Ok => Mark::Step,
        Status::Warn => Mark::Warn,
        Status::Fail => Mark::Error,
    };
    let mut out = display::render_line(mark, &check.name, "", colour);
    for line in check.detail.lines() {
        out.push('\n');
        out.push_str(&display::render_line(Mark::Rail, line, "", colour));
    }
    out
}

pub(super) fn render(checks: &[Check]) {
    let colour = display::Stream::Stdout.colour();
    for check in checks {
        println!("{}", render_check(check, colour));
    }
}

/// Count checks by status: `(ok, warn, fail)`.
pub(super) fn tally(checks: &[Check]) -> (usize, usize, usize) {
    let mut ok = 0;
    let mut warn = 0;
    let mut fail = 0;
    for c in checks {
        match c.status {
            Status::Ok => ok += 1,
            Status::Warn => warn += 1,
            Status::Fail => fail += 1,
        }
    }
    (ok, warn, fail)
}

/// Print the tally and terminate the process with an appropriate exit code.
pub(super) fn summarize_and_exit(checks: &[Check]) {
    let (ok, warn, fail) = tally(checks);
    let counts = format!("{ok} ok, {warn} warning(s), {fail} failure(s)");
    let outro = match (warn, fail) {
        (_, 1..) => "Diagnostics found problems",
        (1.., 0) => "Diagnostics passed with warnings",
        _ => "All checks passed",
    };
    display::outro(outro);
    display::summary(&[("Checks", &counts)]);
    if fail > 0 {
        std::process::exit(1);
    }
}
