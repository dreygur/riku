use std::fs;
use std::path::Path;
use std::time::Duration;

use riku_config::settings::EngineSettings;

use super::run_limit;
use crate::engine::fixture::{logged, logging_engine, sample_spec};
use crate::engine::ContainerId;
use crate::engine::PullPolicy;
use crate::types::{StopSignal, DEFAULT_STOP_GRACE};

const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn run_limit_is_the_pull_limit_only_when_the_run_may_pull() {
    let settings = EngineSettings {
        call_timeout: Duration::from_secs(7),
        pull_timeout: Duration::from_secs(900),
        ..EngineSettings::default()
    };

    assert_eq!(
        run_limit(&settings, PullPolicy::Missing),
        Duration::from_secs(900)
    );
    assert_eq!(
        run_limit(&settings, PullPolicy::Never),
        Duration::from_secs(7)
    );
}

fn id() -> ContainerId {
    ContainerId::try_from(ID).unwrap()
}

#[test]
fn stop_passes_grace_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    engine.stop(&id(), DEFAULT_STOP_GRACE).unwrap();
    engine.stop(&id(), Duration::from_secs(5)).unwrap();
    engine.stop(&id(), Duration::from_millis(500)).unwrap();

    assert_eq!(
        logged(&log),
        [
            format!("stop -t 10 {ID}"),
            format!("stop -t 5 {ID}"),
            format!("stop -t 1 {ID}")
        ]
    );
}

#[test]
fn kill_passes_the_signal() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    engine
        .kill(&id(), &StopSignal::try_from("sigusr1").unwrap())
        .unwrap();

    assert_eq!(logged(&log), [format!("kill --signal SIGUSR1 {ID}")]);
}

#[test]
fn remove_never_deletes_volumes() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    engine.remove(&id()).unwrap();

    assert_eq!(logged(&log), [format!("rm {ID}")]);
}

#[test]
fn run_returns_the_engine_container_id_and_removes_the_env_file() {
    let dir = tempfile::tempdir().unwrap();
    let scratch = dir.path().join("scratch");
    fs::create_dir(&scratch).unwrap();
    let behaviour = format!(
        "case \"$1\" in run) prev=; for a in \"$@\"; do \
         [ \"$prev\" = --env-file ] && cp \"$a\" '{}/env.copy'; prev=$a; done; echo {ID};; esac",
        dir.path().display()
    );
    let (engine, log) = logging_engine(dir.path(), &behaviour);
    let mut spec = sample_spec();
    spec.env
        .insert("DATABASE_URL".into(), "postgres://u:secret@db/x".into());

    let got = engine.run(&spec, &scratch).unwrap();

    assert_eq!(got.as_str(), ID);
    let line = logged(&log).remove(0);
    assert!(!line.contains("secret"), "secret in argv: {line}");
    let delivered = fs::read_to_string(dir.path().join("env.copy")).unwrap();
    assert_eq!(delivered, "DATABASE_URL=postgres://u:secret@db/x\n");
    let env_file = line
        .split("--env-file ")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap();
    assert!(!Path::new(env_file).exists(), "env file left at {env_file}");
}

#[test]
fn run_refuses_an_output_that_is_not_a_container_id() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "echo 'WARNING: slow disk'");

    let err = engine
        .run(&sample_spec(), dir.path())
        .unwrap_err()
        .to_string();

    assert!(err.contains("container id"), "{err}");
}

#[test]
fn run_refuses_an_env_value_with_a_newline_before_calling_the_engine() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");
    let mut spec = sample_spec();
    spec.env.insert("KEY".into(), "a\nb".into());

    let err = engine.run(&spec, dir.path()).unwrap_err().to_string();

    assert!(err.contains("KEY"), "{err}");
    assert!(
        logged(&log).is_empty(),
        "engine was called: {:?}",
        logged(&log)
    );
}

#[test]
fn pull_refuses_an_image_that_looks_like_a_flag() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    assert!(engine
        .pull("--all-tags", &riku_util::display::OutputTail::default())
        .is_err());
    assert!(
        logged(&log).is_empty(),
        "engine was called: {:?}",
        logged(&log)
    );
}

#[test]
fn wait_returns_parsed_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "echo 42");

    let code = engine.wait(&id()).unwrap();

    assert_eq!(code, 42);
    assert_eq!(logged(&log), [format!("wait {ID}")]);
}

#[test]
fn wait_fails_on_unparseable_output() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "echo 'not-a-number'");

    let err = engine.wait(&id()).unwrap_err().to_string();

    assert!(err.contains("cannot parse exit code"), "{err}");
}
