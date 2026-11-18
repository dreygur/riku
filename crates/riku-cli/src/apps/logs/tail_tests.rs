use jiff::civil::date;
use jiff::tz::TimeZone;

use super::prefixed;

#[test]
fn a_stamped_line_shows_its_local_capture_time_before_the_worker() {
    let at = TimeZone::UTC
        .to_timestamp(date(2026, 11, 12).at(8, 14, 3, 0))
        .unwrap();
    let clock = at
        .to_zoned_datetime(TimeZone::system())
        .strftime("%H:%M:%S")
        .to_string();

    let shown = prefixed("web.1", 6, "2026-11-12T08:14:03Z listening on :37139");

    assert_eq!(shown, format!("{clock} web.1  | listening on :37139"));
}

/// No stamp means no time, not a wrong one; the column stays aligned.
#[test]
fn a_line_without_a_stamp_shows_no_time() {
    assert_eq!(
        prefixed("web.1", 5, "listening on :37139"),
        "         web.1 | listening on :37139"
    );
}
