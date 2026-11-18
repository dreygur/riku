use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jiff::civil::{date, DateTime, Weekday};
use jiff::tz::TimeZone;
use jiff::Timestamp;

use super::{calculate_next_run_after, validate_cron_expression};

/// 2024-01-01 00:00:00 UTC, a Monday.
const MONDAY_MIDNIGHT: u64 = 1_704_067_200;

fn next_from_monday(schedule: &str) -> DateTime {
    let after = UNIX_EPOCH + Duration::from_secs(MONDAY_MIDNIGHT);
    let next: SystemTime = calculate_next_run_after(schedule, after).unwrap();
    TimeZone::UTC.to_datetime(Timestamp::try_from(next).unwrap())
}

#[test]
fn weekday_seven_means_sunday() {
    let next = next_from_monday("0 9 * * 7");

    assert_eq!(next.weekday(), Weekday::Sunday, "got {next}");
    assert_eq!(next, date(2024, 1, 7).at(9, 0, 0, 0));
}

#[test]
fn month_and_weekday_names_parse_in_any_case() {
    assert_eq!(
        next_from_monday("0 9 * Feb mon"),
        date(2024, 2, 5).at(9, 0, 0, 0),
        "first Monday of February 2024"
    );
}

/// Vixie cron: with both day fields restricted, either one is enough.
#[test]
fn restricted_day_fields_match_either_day() {
    assert_eq!(
        next_from_monday("0 0 15 * 5"),
        date(2024, 1, 5).at(0, 0, 0, 0),
        "the first Friday comes before the 15th"
    );
}

#[test]
fn rejects_malformed_fields() {
    let malformed = [
        "+5 * * * *",
        "*/+5 * * * *",
        "*/0 * * * *",
        "5/10 * * * *",
        "1,,2 * * * *",
        "5-1 * * * *",
        "60 * * * *",
        "0 0 mon * *",
        "0 0 * * 8",
        "0 0 L * *",
    ];

    for schedule in malformed {
        assert!(!validate_cron_expression(schedule), "accepted {schedule:?}");
    }
}

/// A pre-epoch instant with a fractional second must round down, not
/// toward zero, or the minute just after it is skipped.
#[test]
fn next_run_after_fractional_pre_epoch_instant() {
    let after = UNIX_EPOCH - Duration::from_millis(60_500);
    let next = calculate_next_run_after("* * * * *", after).unwrap();

    assert_eq!(next, UNIX_EPOCH - Duration::from_secs(60));
}
