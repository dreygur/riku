use super::parser::calculate_next_run_after;
use super::*;
use chrono::{Datelike, Timelike, Weekday};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── validate_cron_expression ────────────────────────────────────────────────

#[test]
fn test_validate_cron_expression() {
    assert!(validate_cron_expression("0 * * * *")); // Hourly
    assert!(validate_cron_expression("0 0 * * *")); // Daily at midnight
    assert!(validate_cron_expression("*/5 * * * *")); // Every 5 minutes
    assert!(validate_cron_expression("0 2 * * 1-5")); // 2 AM, Mon-Fri

    assert!(!validate_cron_expression("invalid"));
    assert!(!validate_cron_expression("0 * * *")); // Missing one field
}

#[test]
fn test_validate_cron_expression_ranges() {
    assert!(validate_cron_expression("0-30 6-18 * * *"));
    assert!(validate_cron_expression("*/15 */2 * * *"));
    assert!(validate_cron_expression("5,10,15 * * * *"));
}

#[test]
fn test_validate_cron_expression_invalid_too_few_fields() {
    assert!(!validate_cron_expression(""));
    assert!(!validate_cron_expression("0 * *"));
    assert!(!validate_cron_expression("0 0 1 12")); // only 4 fields
}

#[test]
fn test_validate_cron_expression_rejects_out_of_range() {
    assert!(!validate_cron_expression("60 * * * *")); // minute 60
    assert!(!validate_cron_expression("0 24 * * *")); // hour 24
    assert!(!validate_cron_expression("0 0 * * 8")); // weekday 8
}

#[test]
fn test_validate_cron_expression_rejects_impossible_schedule() {
    // February never has a 30th — the schedule can never fire.
    assert!(!validate_cron_expression("0 0 30 2 *"));
}

// ── calculate_next_run_after ────────────────────────────────────────────────

/// A fixed epoch timestamp for a known point in time.
/// 2024-01-01 00:00:00 UTC (Monday) → unix epoch 1704067200
fn fixed_epoch_monday_midnight() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1704067200)
}

/// Convert a `SystemTime` next-run back into a UTC `DateTime` for assertions.
fn as_utc(time: SystemTime) -> chrono::DateTime<chrono::Utc> {
    let secs = time.duration_since(UNIX_EPOCH).unwrap().as_secs();
    chrono::DateTime::from_timestamp(secs as i64, 0).unwrap()
}

#[test]
fn test_next_run_every_minute() {
    let after = fixed_epoch_monday_midnight();
    let next = calculate_next_run_after("* * * * *", after).unwrap();
    // Should fire one minute later.
    let delta = next.duration_since(after).unwrap();
    assert_eq!(
        delta.as_secs(),
        60,
        "every-minute schedule should advance by 60 s"
    );
}

#[test]
fn test_next_run_hourly() {
    let after = fixed_epoch_monday_midnight(); // 00:00 UTC
    let next = calculate_next_run_after("0 * * * *", after).unwrap();
    // Next run is 01:00 UTC → 3600 s later.
    let delta = next.duration_since(after).unwrap();
    assert_eq!(
        delta.as_secs(),
        3600,
        "hourly schedule should advance by 3600 s from midnight"
    );
}

#[test]
fn test_next_run_invalid_expression_returns_error() {
    let after = fixed_epoch_monday_midnight();
    let result = calculate_next_run_after("not a cron", after);
    assert!(result.is_err(), "invalid cron expression should return Err");
}

#[test]
fn test_next_run_short_expression_returns_error() {
    let after = fixed_epoch_monday_midnight();
    let result = calculate_next_run_after("0 * * *", after); // only 4 fields
    assert!(result.is_err(), "4-field cron expression should return Err");
}

// ── B1: day-of-week restriction must be honoured ─────────────────────────────

#[test]
fn test_next_run_monday_only_fires_on_monday() {
    // `0 9 * * 1` = 09:00 UTC on Mondays. With the old OR-on-wildcard bug this
    // fired every day; the engine must now land on an actual Monday.
    // Start from Tuesday 2024-01-02 00:00 UTC so the very next day is NOT the
    // target weekday — proving non-Mondays are skipped.
    let tuesday = UNIX_EPOCH + Duration::from_secs(1704153600); // 2024-01-02 00:00 UTC (Tue)
    let next = calculate_next_run_after("0 9 * * 1", tuesday).unwrap();
    let dt = as_utc(next);

    assert_eq!(
        dt.weekday(),
        Weekday::Mon,
        "0 9 * * 1 must fire on a Monday, got {dt}"
    );
    assert_eq!(dt.hour(), 9, "must fire at 09:00 UTC");
    assert_eq!(dt.minute(), 0);
    // From Tuesday the next Monday is 2024-01-08.
    assert_eq!(dt.day(), 8, "next Monday after 2024-01-02 is the 8th");
}

// ── B4: unsatisfiable schedule must error, not silently fall back ────────────

#[test]
fn test_next_run_impossible_schedule_returns_error() {
    let after = fixed_epoch_monday_midnight();
    // Feb 30th never exists.
    let result = calculate_next_run_after("0 0 30 2 *", after);
    assert!(
        result.is_err(),
        "impossible schedule must return Err, not a fallback time"
    );
}

// Range/list schedule the OLD Procfile regex would have rejected.
#[test]
fn test_next_run_weekday_range() {
    // 2024-01-06 00:00 UTC is a Saturday; the next Mon–Fri 09:00 slot is
    // Monday 2024-01-08 09:00 UTC.
    let saturday = UNIX_EPOCH + Duration::from_secs(1704499200);
    let next = calculate_next_run_after("0 9 * * 1-5", saturday).unwrap();
    let dt = as_utc(next);
    assert_eq!(dt.weekday(), Weekday::Mon);
    assert_eq!(dt.day(), 8);
    assert_eq!(dt.hour(), 9);
}

// ── CronJob ─────────────────────────────────────────────────────────────────

#[test]
fn test_cron_job_creation() {
    let job = CronJob::new(
        "testapp".to_string(),
        "0 * * * *".to_string(),
        "echo 'hello'".to_string(),
    )
    .unwrap();

    assert_eq!(job.app, "testapp");
    assert_eq!(job.schedule, "0 * * * *");
    assert_eq!(job.command, "echo 'hello'");
}

#[test]
fn test_cron_job_creation_with_invalid_schedule_fails() {
    let result = CronJob::new(
        "testapp".to_string(),
        "bad schedule".to_string(),
        "echo hi".to_string(),
    );
    assert!(result.is_err(), "CronJob with invalid schedule should fail");
}

#[test]
fn test_cron_scheduler_add_remove_job() {
    let mut scheduler = CronScheduler::new();
    scheduler
        .add_job("myapp", 0, "*/5 * * * *", "echo tick")
        .unwrap();
    scheduler
        .add_job("myapp", 1, "0 9 * * *", "echo daily")
        .unwrap();
    scheduler
        .add_job("other", 0, "*/5 * * * *", "echo other")
        .unwrap();
    assert_eq!(scheduler.get_jobs().len(), 3);

    // remove_app_jobs purges every job for the app, leaving other apps intact.
    scheduler.remove_app_jobs("myapp");
    assert_eq!(scheduler.get_jobs().len(), 1);
    assert!(scheduler.get_jobs().contains_key("other-cron-0"));
}

#[test]
fn test_cron_scheduler_mark_job_run_advances_next_run() {
    let mut scheduler = CronScheduler::new();
    scheduler
        .add_job("myapp", 0, "*/5 * * * *", "echo tick")
        .unwrap();

    let before = scheduler.get_jobs()["myapp-cron-0"].next_run;
    scheduler.mark_job_run("myapp", 0).unwrap();
    let after = scheduler.get_jobs()["myapp-cron-0"].next_run;

    assert!(after > before, "next_run should advance after mark_job_run");
}
