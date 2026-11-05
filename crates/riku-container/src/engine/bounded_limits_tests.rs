use std::time::{Duration, Instant};

use riku_config::settings::EngineSettings;

use crate::engine::fixture::{logging_engine, ID};
use crate::engine::{BuildSpec, ContainerId};

fn short_limits() -> EngineSettings {
    EngineSettings {
        call_timeout: Duration::from_millis(300),
        stop_extra: Duration::from_millis(300),
        pull_timeout: Duration::from_millis(300),
        build_timeout: Duration::from_millis(300),
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

    let message = err.to_string();
    assert!(
        message.contains("'pull'")
            && message.contains("300ms")
            && message.contains("registry.example/myapp:1"),
        "error should name pull, the image and the limit: {message}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "pull was not stopped"
    );
}

#[test]
fn build_limit_applies() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in build) exec sleep 30;; esac");
    let engine = engine.with_settings(short_limits());
    let spec = BuildSpec {
        context: dir.path().to_path_buf(),
        dockerfile: None,
        tag: "myapp-web:1".into(),
        args: Default::default(),
        platform: None,
    };

    let err = engine.build(&spec).unwrap_err();

    let message = err.to_string();
    assert!(
        message.contains("'build'") && message.contains("300ms") && message.contains("myapp-web:1"),
        "error should name build, the tag and the limit: {message}"
    );
}

#[test]
fn failed_pull_names_the_image_and_status() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in pull) exit 3;; esac");

    let err = engine.pull("registry.example/myapp:1").unwrap_err();

    assert!(
        err.to_string().contains("registry.example/myapp:1") && err.to_string().contains('3'),
        "error should name the image and exit status: {err}"
    );
}

#[test]
fn login_limit_applies() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in login) exec sleep 30;; esac");
    let engine = engine.with_settings(short_limits());

    let err = engine
        .login("registry.example", "deploy", "token")
        .unwrap_err();

    assert!(
        err.to_string().contains("'login'") && err.to_string().contains("300ms"),
        "error should name login and the limit: {err}"
    );
}

#[test]
fn wait_uses_the_build_limit_not_the_call_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "case \"$1\" in wait) sleep 1; echo 7;; esac");
    let engine = engine.with_settings(EngineSettings {
        call_timeout: Duration::from_millis(200),
        stop_extra: Duration::from_millis(300),
        pull_timeout: Duration::from_millis(300),
        build_timeout: Duration::from_secs(5),
    });

    assert_eq!(
        engine.wait(&ContainerId::try_from(ID).unwrap()).ok(),
        Some(7),
        "wait should succeed using build limit rather than call limit"
    );
}
