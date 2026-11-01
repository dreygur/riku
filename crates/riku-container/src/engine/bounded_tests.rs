use std::ffi::OsString;
use std::fs;
use std::time::{Duration, Instant};

use riku_config::settings::EngineSettings;

use crate::engine::fake::fake_engine;
use crate::engine::fixture::{logging_engine, ID};
use crate::engine::{BuildSpec, ContainerId};

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

fn wait_until_gone(pid: &str) -> bool {
    (0..40).any(|_| {
        let gone = !pid_alive(pid);
        if !gone {
            std::thread::sleep(Duration::from_millis(50));
        }
        gone
    })
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

    let out = engine
        .output_within(&[OsString::from("ps")], Duration::from_millis(500))
        .unwrap();

    assert!(out.contains("ok"), "output should contain 'ok': {out}");
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
