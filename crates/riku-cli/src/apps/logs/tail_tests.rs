use chrono::{Local, TimeZone, Utc};

use super::prefixed;

#[test]
fn a_stamped_line_shows_its_local_capture_time_before_the_worker() {
    let at = Utc.with_ymd_and_hms(2026, 11, 12, 8, 14, 3).unwrap();
    let clock = at.with_timezone(&Local).format("%H:%M:%S").to_string();

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
