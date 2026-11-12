use super::*;

#[test]
fn constant_time_eq_matches_normal_equality() {
    assert!(constant_time_eq("abc123", "abc123"));
    assert!(!constant_time_eq("abc123", "abc124"));
    assert!(!constant_time_eq("short", "longer-string"));
    assert!(constant_time_eq("", ""));
}
