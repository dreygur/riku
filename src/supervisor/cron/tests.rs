use super::parser::{calculate_next_run_after, parse_cron_field};
use super::*;
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

// ── parse_cron_field ────────────────────────────────────────────────────────

#[test]
fn test_parse_cron_field_wildcard() {
    let vals = parse_cron_field("*", 0, 4).unwrap();
    assert_eq!(vals, vec![0, 1, 2, 3, 4]);
}

#[test]
fn test_parse_cron_field_single_value() {
    let vals = parse_cron_field("5", 0, 59).unwrap();
    assert_eq!(vals, vec![5]);
}

#[test]
fn test_parse_cron_field_range() {
    let vals = parse_cron_field("1-3", 0, 59).unwrap();
    assert_eq!(vals, vec![1, 2, 3]);
}

#[test]
fn test_parse_cron_field_step() {
    let vals = parse_cron_field("*/10", 0, 59).unwrap();
    assert_eq!(vals, vec![0, 10, 20, 30, 40, 50]);
}

#[test]
fn test_parse_cron_field_list() {
    let mut vals = parse_cron_field("1,3,5", 0, 59).unwrap();
    vals.sort();
    assert_eq!(vals, vec![1, 3, 5]);
}

#[test]
fn test_parse_cron_field_step_from_value() {
    // "5/15" means start at 5, step by 15 → 5, 20, 35, 50
    let vals = parse_cron_field("5/15", 0, 59).unwrap();
    assert_eq!(vals, vec![5, 20, 35, 50]);
}

#[test]
fn test_parse_cron_field_range_with_step() {
    // "10-40/10" means 10..40 step 10 → 10, 20, 30, 40
    let vals = parse_cron_field("10-40/10", 0, 59).unwrap();
    assert_eq!(vals, vec![10, 20, 30, 40]);
}

// ── calculate_next_run_after ────────────────────────────────────────────────

/// A fixed epoch timestamp for a known point in time.
/// 2024-01-01 00:00:00 UTC (Monday) → unix epoch 1704067200
fn fixed_epoch_monday_midnight() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1704067200)
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
    assert_eq!(scheduler.get_jobs().len(), 1);

    scheduler.remove_job("myapp", 0).unwrap();
    assert_eq!(scheduler.get_jobs().len(), 0);
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
