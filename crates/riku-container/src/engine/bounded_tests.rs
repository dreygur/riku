use std::ffi::OsString;
use std::fs;
use std::time::{Duration, Instant};

use crate::engine::fake::fake_engine;
use crate::engine::fixture::{logging_engine, pid_alive, wait_until_gone};

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

#[test]
fn timeout_stops_helpers_that_ignore_sigterm() {
    let dir = tempfile::tempdir().unwrap();
    let leader_pid = dir.path().join("leader.pid");
    let helper_pid = dir.path().join("helper.pid");
    let helper = fake_engine(
        dir.path(),
        "helper",
        &format!(
            "trap '' TERM\necho $$ > '{}'\nexec sleep 30",
            helper_pid.display()
        ),
    );
    let behaviour = format!(
        "'{}' &\nwhile [ ! -s '{}' ]; do sleep 0.05; done\necho $$ > '{}'\nexec sleep 30",
        helper.display(),
        helper_pid.display(),
        leader_pid.display()
    );
    let (engine, _log) = logging_engine(dir.path(), &behaviour);

    let err = engine
        .output_within(&[OsString::from("ps")], Duration::from_secs(3))
        .unwrap_err();

    assert!(err.to_string().contains("'ps'"), "{err}");
    for (what, file) in [("engine client", &leader_pid), ("helper", &helper_pid)] {
        let pid = fs::read_to_string(file).unwrap();
        assert!(
            wait_until_gone(pid.trim()),
            "{what} {} still running after the timeout",
            pid.trim()
        );
    }
}

#[test]
fn capture_returns_when_a_helper_keeps_stdout_open() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("helper.pid");
    let behaviour = format!(
        "sleep 30 &\necho $! > '{}'\necho ok\nexit 0",
        pid_file.display()
    );
    let (engine, _log) = logging_engine(dir.path(), &behaviour);
    let started = Instant::now();

    let res = engine.run_within(&[OsString::from("ps")], Duration::from_millis(500));
    assert!(res.is_ok(), "run_within should succeed: {res:?}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "capture waited for the helper"
    );
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(
        wait_until_gone(pid.trim()),
        "helper {} still running",
        pid.trim()
    );
}

#[test]
fn capture_flags_output_cut_by_a_lingering_helper() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("helper.pid");
    let behaviour = format!(
        "sleep 30 &\necho $! > '{}'\necho ok\nexit 0",
        pid_file.display()
    );
    let (engine, _log) = logging_engine(dir.path(), &behaviour);
    let limit = Duration::from_millis(500);

    let captured = super::capture(engine.program(), &[], &[OsString::from("ps")], limit).unwrap();

    assert!(captured.truncated, "output should be flagged as truncated");
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(
        wait_until_gone(pid.trim()),
        "helper {} still running",
        pid.trim()
    );
}

#[test]
fn capture_of_a_clean_exit_is_not_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "echo ok");
    let limit = Duration::from_millis(500);

    let captured = super::capture(engine.program(), &[], &[OsString::from("ps")], limit).unwrap();

    assert!(!captured.truncated, "clean exit should not be truncated");
    let stdout = String::from_utf8_lossy(&captured.stdout);
    assert!(
        stdout.contains("ok"),
        "output should contain 'ok': {stdout}"
    );
}
