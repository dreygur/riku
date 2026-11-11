use std::time::{Duration, Instant};

use super::*;

/// A release command that leaves a background child holding its output
/// pipe must not hold the deploy, and the app's deploy lock, open.
#[test]
fn release_returns_when_a_background_child_keeps_the_pipe_open() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();

    let result = run_one_shot(
        OneShot::Release,
        "sleep 30 & echo migrated",
        dir.path(),
        &OutputTail::default(),
    );

    assert!(result.is_ok(), "{result:?}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "waited {:?} for a background child",
        started.elapsed()
    );
}
