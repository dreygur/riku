use std::ffi::OsString;
use std::fs;
use std::time::Duration;

use riku_config::settings::EngineSettings;

use crate::engine::fixture::{logging_engine, wait_until_gone, ID};
use crate::engine::ContainerId;

#[test]
fn output_within_errors_when_capture_is_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("helper.pid");
    let behaviour = format!(
        "sleep 30 &\necho $! > '{}'\necho 7\nexit 0",
        pid_file.display()
    );
    let (engine, _log) = logging_engine(dir.path(), &behaviour);

    let err = engine
        .output_within(&[OsString::from("ps")], Duration::from_millis(500))
        .unwrap_err();

    let message = err.to_string();
    assert!(
        message.contains("'ps'") && message.contains("output may be incomplete"),
        "error should name subcommand and state output may be incomplete: {message}"
    );
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(
        wait_until_gone(pid.trim()),
        "helper {} still running",
        pid.trim()
    );
}

#[test]
fn wait_errors_when_capture_is_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("helper.pid");
    let behaviour = format!(
        "sleep 30 &\necho $! > '{}'\necho 7\nexit 0",
        pid_file.display()
    );
    let (engine, _log) = logging_engine(dir.path(), &behaviour);
    let settings = EngineSettings {
        build_timeout: Duration::from_millis(500),
        ..EngineSettings::default()
    };
    let engine = engine.with_settings(settings);
    let id = ContainerId::try_from(ID).unwrap();

    let err = engine.wait(&id).unwrap_err();

    let message = err.to_string();
    assert!(
        message.contains("'wait'") && message.contains("output may be incomplete"),
        "wait should error on truncated output: {message}"
    );
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(
        wait_until_gone(pid.trim()),
        "helper {} still running",
        pid.trim()
    );
}

#[test]
fn run_within_succeeds_when_capture_is_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("helper.pid");
    let behaviour = format!(
        "sleep 30 &\necho $! > '{}'\necho 7\nexit 0",
        pid_file.display()
    );
    let (engine, _log) = logging_engine(dir.path(), &behaviour);

    let result = engine.run_within(&[OsString::from("stop")], Duration::from_millis(500));

    assert!(
        result.is_ok(),
        "run_within should succeed despite truncated output: {result:?}"
    );
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(
        wait_until_gone(pid.trim()),
        "helper {} still running",
        pid.trim()
    );
}
