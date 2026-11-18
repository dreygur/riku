use jiff::civil::date;
use jiff::tz::TimeZone;

use super::*;

fn at(h: i8, m: i8, s: i8) -> Timestamp {
    TimeZone::UTC
        .to_timestamp(date(2026, 11, 12).at(h, m, s, 0))
        .unwrap()
}

#[test]
fn a_stamped_line_splits_back_into_its_time_and_text() {
    let line = stamp_line(at(8, 14, 3), "listening on :37139");

    assert_eq!(line, "2026-11-12T08:14:03Z listening on :37139");
    assert_eq!(
        split_stamp(&line),
        Some((at(8, 14, 3), "listening on :37139"))
    );
}

/// deploy.log keeps its own `[<local time with offset>]` prefix.
#[test]
fn a_deploy_log_line_gives_its_time_in_utc() {
    assert_eq!(
        split_stamp("[2026-11-12T14:14:03+0600] Syncing repo to HEAD"),
        Some((at(8, 14, 3), "Syncing repo to HEAD"))
    );
}

/// App output that only looks like a date, or carries no stamp at all, has
/// no time: showing one would be a guess.
#[test]
fn a_line_without_a_stamp_has_no_time() {
    for line in [
        "listening on :37139",
        "[WARN] build cache missing",
        "2026-11-12 hello",
        "",
    ] {
        assert_eq!(split_stamp(line), None, "{line:?}");
    }
}
