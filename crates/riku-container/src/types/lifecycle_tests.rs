use super::*;
use std::time::Duration;

#[test]
fn stop_signal_defaults_to_sigterm_and_rejects_junk() {
    assert_eq!(StopSignal::default().as_str(), "SIGTERM");
    assert_eq!(StopSignal::try_from("sigint").unwrap().as_str(), "SIGINT");
    assert!(StopSignal::try_from("TERM; rm").is_err());
    assert_eq!(DEFAULT_STOP_GRACE, Duration::from_secs(30));
}

#[test]
fn restart_policy_defaults_to_no() {
    assert_eq!(RestartPolicy::default(), RestartPolicy::No);
}
