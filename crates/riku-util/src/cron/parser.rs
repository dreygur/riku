//! Cron expression parsing and next-run time calculation.
//!
//! # Schedule contract
//!
//! Schedules use the **5-field Unix cron** format that users write in a
//! Procfile: `minute hour day-of-month month day-of-week`. This is the same
//! grammar the public [`crate::util::procfile`] parser accepts, both route
//! through [`validate_cron_expression`] here so there is a single source of
//! truth for what a valid schedule is. The field syntax is listed in
//! `field.rs`.
//!
//! # Time zone (important)
//!
//! All schedules are interpreted in **UTC**, never the host's local time. A
//! schedule of `0 9 * * 1` fires at 09:00 UTC on Mondays regardless of the
//! server's `TZ`. This is intentional and stable across daylight-saving
//! transitions; operators who need a local wall-clock time must offset their
//! schedule manually.
//!
//! # Day-of-month vs day-of-week semantics
//!
//! When **both** the day-of-month and day-of-week fields are restricted, a
//! match occurs if **either** field matches (logical OR). When at least one
//! of the two is a bare `*`, the fields are combined with logical AND. This
//! means `0 9 * * 1` correctly fires only on Mondays. Anything other than a
//! bare `*` counts as restricted, `*/2` included, as croner did. Vixie cron
//! treats a field starting with `*` as unrestricted, so `0 9 */2 * 1` fires
//! on odd days and on Mondays here, but only on odd day Mondays there.

#![allow(clippy::question_mark)]

use anyhow::{anyhow, Result};
use std::time::SystemTime;

use jiff::Timestamp;

use super::schedule::Schedule;

fn parse_schedule(schedule: &str) -> Result<Schedule> {
    match schedule.parse::<Schedule>() {
        Ok(parsed) => Ok(parsed),
        Err(e) => Err(anyhow!("Invalid cron expression '{}': {}", schedule, e)),
    }
}

/// Calculate the next run time strictly after the current instant.
pub(super) fn calculate_next_run(schedule: &str) -> Result<SystemTime> {
    calculate_next_run_after(schedule, SystemTime::now())
}

/// Calculate the next UTC fire time strictly **after** the given instant.
///
/// Returns an error for invalid schedules and for unsatisfiable schedules
/// (e.g. `0 0 30 2 *`: February never has a 30th), instead of silently
/// degrading to an hourly fallback.
pub(super) fn calculate_next_run_after(schedule: &str, after: SystemTime) -> Result<SystemTime> {
    let parsed = match parse_schedule(schedule) {
        Ok(parsed) => parsed,
        Err(e) => return Err(e),
    };
    let after = match Timestamp::try_from(after) {
        Ok(after) => after,
        Err(e) => return Err(anyhow!("Time out of range: {e}")),
    };

    let next = match parsed.next_after(after) {
        Ok(next) => next,
        Err(e) => {
            return Err(anyhow!(
                "Schedule '{}' has no upcoming run: {}",
                schedule,
                e
            ))
        }
    };
    match SystemTime::try_from(next) {
        Ok(next) => Ok(next),
        Err(e) => Err(anyhow!("Next run time out of range: {e}")),
    }
}

/// Validate a cron expression: returns `true` only for a well-formed 5-field
/// schedule that the engine can actually satisfy. This is the single
/// validator shared with the Procfile parser.
pub fn validate_cron_expression(expr: &str) -> bool {
    let Ok(schedule) = parse_schedule(expr) else {
        return false;
    };

    // A grammatically valid but unsatisfiable schedule (e.g. `0 0 30 2 *`)
    // must be rejected. Probe from the epoch; if no occurrence exists, the
    // schedule can never fire.
    schedule.next_after(Timestamp::UNIX_EPOCH).is_ok()
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
