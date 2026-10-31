use std::ffi::OsString;
use std::fs;
use std::time::{Duration, Instant};

use riku_config::settings::EngineSettings;

use crate::engine::fixture::{logging_engine, ID};
use crate::engine::ContainerId;

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

fn short_limits() -> EngineSettings {
    EngineSettings {
        stop_extra: Duration::from_millis(300),
        pull_timeout: Duration::from_millis(300),
        build_timeout: Duration::from_millis(300),
        ..EngineSettings::default()
    }
}

#[test]
fn stop_waits_grace_plus_extra() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in stop) exec sleep 30;; esac");
    let engine = engine.with_settings(short_limits());
    let id = ContainerId::try_from(ID).unwrap();
    let started = Instant::now();

    let err = engine.stop(&id, Duration::from_secs(1)).unwrap_err();

    let waited = started.elapsed();
    assert!(
        err.to_string().contains("'stop'"),
        "error should name the subcommand: {err}"
    );
    assert!(
        waited >= Duration::from_millis(1300) && waited < Duration::from_secs(10),
        "stop should be killed after grace plus extra, waited {waited:?}"
    );
}

#[test]
fn pull_limit_applies() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in pull) exec sleep 30;; esac");
    let engine = engine.with_settings(short_limits());
    let started = Instant::now();

    let err = engine.pull("registry.example/myapp:1").unwrap_err();

    assert!(
        err.to_string().contains("'pull'") && err.to_string().contains("300ms"),
        "error should name pull and the limit: {err}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "pull was not stopped"
    );
}
