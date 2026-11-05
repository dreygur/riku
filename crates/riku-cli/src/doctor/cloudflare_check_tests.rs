use std::time::{Duration, SystemTime};

use super::*;
use crate::doctor::Status;

#[test]
fn warns_when_missing_or_older_than_thirty_days() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("cloudflare.conf");
    let missing = cloudflare_ranges(&file, SystemTime::now());
    std::fs::write(&file, "allow 192.0.2.0/24;\n").unwrap();
    let now = SystemTime::now();
    let fresh = cloudflare_ranges(&file, now);
    let in_29_days = cloudflare_ranges(&file, now + Duration::from_secs(29 * 86400));
    let in_31_days = cloudflare_ranges(&file, now + Duration::from_secs(31 * 86400));

    assert_eq!(missing.status, Status::Warn, "{}", missing.detail);
    assert_eq!(fresh.status, Status::Ok, "{}", fresh.detail);
    assert_eq!(in_29_days.status, Status::Ok, "{}", in_29_days.detail);
    assert_eq!(in_31_days.status, Status::Warn, "{}", in_31_days.detail);
    assert!(
        in_31_days.detail.contains("31 days"),
        "{}",
        in_31_days.detail
    );
}
