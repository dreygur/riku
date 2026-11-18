use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jiff::civil::{date, DateTime, Weekday};
use jiff::tz::TimeZone;
use jiff::Timestamp;

use super::calculate_next_run_after;

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
