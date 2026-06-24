//! Cron expression parsing and next-run time calculation.
//!
//! # Schedule contract
//!
//! Schedules use the **5-field Unix cron** format that users write in a
//! Procfile: `minute hour day-of-month month day-of-week`. This is the same
//! grammar the public [`crate::util::procfile`] parser accepts — both route
//! through [`validate_cron_expression`] here so there is a single source of
//! truth for what a valid schedule is.
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
//! Standard Vixie-cron semantics are used (provided by the [`croner`] crate):
//! when **both** the day-of-month and day-of-week fields are restricted (i.e.
//! neither is `*`), a match occurs if **either** field matches (logical OR).
//! When at least one of the two is `*`, the fields are combined with logical
//! AND. This means `0 9 * * 1` correctly fires only on Mondays.

use anyhow::{anyhow, Result};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use croner::Cron;

/// Parse a 5-field Unix cron expression with Vixie day-of-month/day-of-week
/// semantics. Returns an error for any malformed or out-of-range field.
fn parse_schedule(schedule: &str) -> Result<Cron> {
    // Reject anything that is not exactly five whitespace-separated fields so
    // the public 5-field contract cannot drift (croner itself also accepts
    // 6-field "with seconds" patterns, which we deliberately do not expose).
    if schedule.split_whitespace().count() != 5 {
        return Err(anyhow!(
            "Invalid cron expression (expected 5 fields): {}",
            schedule
        ));
    }

    Cron::new(schedule)
        .parse()
        .map_err(|e| anyhow!("Invalid cron expression '{}': {}", schedule, e))
}

/// Convert a [`SystemTime`] into a UTC [`DateTime`] for croner.
fn system_time_to_utc(time: SystemTime) -> Result<DateTime<Utc>> {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| anyhow!("Time is before the Unix epoch"))?
        .as_secs();
    DateTime::from_timestamp(secs as i64, 0).ok_or_else(|| anyhow!("Timestamp out of range"))
}

/// Convert a UTC [`DateTime`] back into a [`SystemTime`].
fn utc_to_system_time(time: DateTime<Utc>) -> Result<SystemTime> {
    let secs = time.timestamp();
    if secs < 0 {
        return Err(anyhow!("Next run time is before the Unix epoch"));
    }
    Ok(UNIX_EPOCH + Duration::from_secs(secs as u64))
}

/// Calculate the next run time strictly after the current instant.
pub(super) fn calculate_next_run(schedule: &str) -> Result<SystemTime> {
    calculate_next_run_after(schedule, SystemTime::now())
}

/// Calculate the next UTC fire time strictly **after** the given instant.
///
/// Returns an error for invalid schedules and for unsatisfiable schedules
/// (e.g. `0 0 30 2 *` — February never has a 30th), instead of silently
/// degrading to an hourly fallback.
pub(super) fn calculate_next_run_after(schedule: &str, after: SystemTime) -> Result<SystemTime> {
    let cron = parse_schedule(schedule)?;
    let after_utc = system_time_to_utc(after)?;

    // `false` => the occurrence must be strictly after `after_utc`.
    let next = cron
        .find_next_occurrence(&after_utc, false)
        .map_err(|e| anyhow!("Schedule '{}' has no upcoming run: {}", schedule, e))?;

    utc_to_system_time(next)
}

/// Validate a cron expression: returns `true` only for a well-formed 5-field
/// schedule that the engine can actually satisfy. This is the single
/// validator shared with the Procfile parser.
pub fn validate_cron_expression(expr: &str) -> bool {
    let Ok(cron) = parse_schedule(expr) else {
        return false;
    };

    // A grammatically valid but unsatisfiable schedule (e.g. `0 0 30 2 *`)
    // must be rejected. Probe from the epoch; if no occurrence exists, the
    // schedule can never fire.
    cron.find_next_occurrence(&DateTime::<Utc>::UNIX_EPOCH, true)
        .is_ok()
}
