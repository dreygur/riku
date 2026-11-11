use std::collections::HashMap;
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
        &HashMap::new(),
        &OutputTail::default(),
    );

    assert!(result.is_ok(), "{result:?}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "waited {:?} for a background child",
        started.elapsed()
    );
}

#[test]
fn release_runs_under_the_open_files_limit_its_app_sets() {
    let dir = tempfile::tempdir().unwrap();
    let app_env = HashMap::from([("RIKU_MAX_OPEN_FILES".to_string(), "200".to_string())]);

    let result = run_one_shot(
        OneShot::Release,
        "ulimit -n > nofile",
        dir.path(),
        &app_env,
        &OutputTail::default(),
    );

    assert!(result.is_ok(), "{result:?}");
    let written = std::fs::read_to_string(dir.path().join("nofile")).unwrap();
    assert_eq!(written.trim(), "200");
}
