use std::process::Stdio;
use std::thread::sleep;
use std::time::{Duration, Instant};

use riku_config::settings::EngineSettings;

use crate::engine::fixture::{logging_engine, ID};
use crate::engine::ContainerId;

fn short_limits() -> EngineSettings {
    EngineSettings {
        call_timeout: Duration::from_millis(300),
        stop_extra: Duration::from_millis(300),
        pull_timeout: Duration::from_millis(300),
        build_timeout: Duration::from_millis(300),
    }
}

#[test]
fn print_logs_is_bounded_by_the_call_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in logs) exec sleep 30;; esac");
    let engine = engine.with_settings(short_limits());
    let id = ContainerId::try_from(ID).unwrap();
    let started = Instant::now();

    let err = match engine.print_logs(&id) {
        Ok(()) => panic!("print_logs should have timed out"),
        Err(e) => e,
    };

    let message = err.to_string();
    assert!(
        message.contains("logs"),
        "error should name 'logs': {message}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "print_logs was not stopped within 10 s"
    );
}

#[test]
fn follow_logs_are_not_cut_by_the_call_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in logs) exec sleep 30;; esac");
    let engine = engine.with_settings(short_limits());
    let id = ContainerId::try_from(ID).unwrap();

    let mut command = engine.logs_command(&id, None);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => panic!("cannot spawn logs command: {e}"),
    };
    sleep(Duration::from_millis(900));

    assert_eq!(
        child.try_wait().unwrap(),
        None,
        "logs follower was cut by call limit"
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn remove_is_bounded_by_the_call_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in rm) exec sleep 30;; esac");
    let engine = engine.with_settings(short_limits());
    let id = ContainerId::try_from(ID).unwrap();
    let started = Instant::now();

    let err = match engine.remove(&id) {
        Ok(()) => panic!("remove should have timed out"),
        Err(e) => e,
    };

    let message = err.to_string();
    assert!(message.contains("rm"), "error should name 'rm': {message}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "remove was not stopped within 10 s"
    );
}
