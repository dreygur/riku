use std::time::{Duration, Instant};

use riku_config::settings::EngineSettings;
use riku_util::display::OutputTail;

use crate::engine::fixture::{logged, logging_engine, ID};
use crate::engine::{ContainerId, LogRange};

fn short_limits() -> EngineSettings {
    EngineSettings {
        program: None,
        call_timeout: Duration::from_millis(300),
        stop_extra: Duration::from_millis(300),
        pull_timeout: Duration::from_millis(300),
        build_timeout: Duration::from_millis(300),
    }
}

#[test]
fn print_logs_is_bounded_by_the_build_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(
        dir.path(),
        "case \"$1\" in logs) echo line; sleep 1; exit 0;; esac",
    );
    let mut limits = short_limits();
    limits.call_timeout = Duration::from_millis(200);
    limits.build_timeout = Duration::from_secs(3);
    let engine = engine.with_settings(limits);
    let id = ContainerId::try_from(ID).unwrap();

    let result = engine.print_logs(&id, &OutputTail::default());
    assert!(
        result.is_ok(),
        "print_logs should succeed within build limit: {result:?}"
    );

    let dir_hang = tempfile::tempdir().unwrap();
    let (engine, _log) =
        logging_engine(dir_hang.path(), "case \"$1\" in logs) exec sleep 30;; esac");
    let mut limits = short_limits();
    limits.build_timeout = Duration::from_millis(600);
    let engine = engine.with_settings(limits);
    let started = Instant::now();

    let err = match engine.print_logs(&id, &OutputTail::default()) {
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
fn timestamped_logs_asks_since_the_bound_and_keeps_both_streams() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(
        dir.path(),
        "case \"$1\" in logs) echo 'T1 out'; echo 'T2 err' >&2;; esac",
    );
    let id = ContainerId::try_from(ID).unwrap();
    let since = LogRange::Since("2026-11-04T18:13:27.125923011Z".to_string());

    let logs = engine.timestamped_logs(&id, &since, 1024).unwrap();

    assert_eq!(logs.stdout, "T1 out\n");
    assert_eq!(logs.stderr, "T2 err\n");
    assert_eq!(
        logged(&log),
        [format!(
            "logs --timestamps --since 2026-11-04T18:13:27.125923011Z {ID}"
        )]
    );
}

#[test]
fn failed_logs_call_reports_only_a_short_engine_line() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(
        dir.path(),
        "case \"$1\" in logs) \
         echo '2026-11-04T10:00:00.000000001Z secret container output' >&2; \
         printf 'Error: %0300d\\n' 0 >&2; \
         echo '2026-11-04T10:00:00.000000002Z more container output' >&2; \
         exit 1;; esac",
    );
    let id = ContainerId::try_from(ID).unwrap();

    let err = engine
        .timestamped_logs(&id, &LogRange::All, 1024)
        .unwrap_err()
        .to_string();

    assert!(
        !err.contains("container output"),
        "leaked container stderr: {err}"
    );
    assert!(err.contains("exit status: 1"), "no exit status in {err}");
    let expected: String = format!("Error: {}", "0".repeat(300))
        .chars()
        .take(200)
        .collect();
    assert!(
        err.ends_with(&format!("{expected:?}")),
        "detail not cut to 200 chars: {err}"
    );
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
