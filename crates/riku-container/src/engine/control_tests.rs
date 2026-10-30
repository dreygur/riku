use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::BuildSpec;
use crate::engine::fixture::{logged, logging_engine, sample_spec};
use crate::engine::ContainerId;
use crate::types::{StopSignal, DEFAULT_STOP_GRACE};

const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn id() -> ContainerId {
    ContainerId::try_from(ID).unwrap()
}

#[test]
fn stop_passes_grace_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    engine.stop(&id(), DEFAULT_STOP_GRACE).unwrap();
    engine.stop(&id(), Duration::from_secs(5)).unwrap();

    assert_eq!(
        logged(&log),
        [format!("stop -t 30 {ID}"), format!("stop -t 5 {ID}")]
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
fn build_orders_flags_before_the_context() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");
    let spec = BuildSpec {
        context: PathBuf::from("/srv/app"),
        dockerfile: Some(PathBuf::from("Dockerfile.prod")),
        tag: "myapp-web:1".into(),
        args: BTreeMap::from([("RUBY_VERSION".to_string(), "3.3".to_string())]),
        platform: Some("linux/amd64".into()),
    };

    engine.build(&spec).unwrap();

    assert_eq!(
        logged(&log),
        ["build -t myapp-web:1 -f Dockerfile.prod --build-arg RUBY_VERSION=3.3 --platform linux/amd64 /srv/app"]
    );
}

#[test]
fn refuses_image_and_build_context_that_look_like_flags() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");
    let spec = BuildSpec {
        context: PathBuf::from("--output=/etc"),
        dockerfile: None,
        tag: "myapp-web:1".into(),
        args: BTreeMap::new(),
        platform: None,
    };

    assert!(engine.pull("--all-tags").is_err());
    assert!(engine.build(&spec).is_err());
    assert!(
        logged(&log).is_empty(),
        "engine was called: {:?}",
        logged(&log)
    );
}

#[test]
fn logs_command_follows_and_tails() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "");

    let command = engine.logs_command(&id(), true, Some(50));

    assert_eq!(command.get_program(), engine.program().as_os_str());
    let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy()).collect();
    assert_eq!(args, ["logs", "-f", "--tail", "50", ID]);
}
