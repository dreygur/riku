use std::ffi::OsString;
use std::fs;
use std::time::{Duration, Instant};

use crate::engine::fixture::logging_engine;

fn pid_alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[test]
fn hung_engine_call_is_killed_at_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let behaviour = format!("echo $$ > '{}'; exec sleep 30", pid_file.display());
    let (engine, _log) = logging_engine(dir.path(), &behaviour);
    let started = Instant::now();

    let err = engine
        .output_within(&[OsString::from("ps")], Duration::from_millis(300))
        .unwrap_err();

    let message = err.to_string();
    assert!(
        message.contains("'ps'") && message.contains("300ms"),
        "error should name the subcommand and limit: {message}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "did not stop at the limit"
    );
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(
        !pid_alive(pid.trim()),
        "engine child {} still running",
        pid.trim()
    );
}
