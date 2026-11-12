use std::fs;
use std::time::Duration;

use super::*;
use crate::config::RikuPaths;
use crate::mock_engine::{mock_engine, socket_refusing_engine};

/// Docker 29 names the socket by its path, not the word "socket".
#[test]
fn docker_29_socket_refusal_names_both_choices() {
    let err = EngineUnreachable::new(
        "cannot read the engine version: container engine 'version' failed: \
         permission denied while trying to connect to the docker API at \
         unix:///var/run/docker.sock"
            .to_string(),
    );

    assert!(
        err.to_string().contains("Choose one"),
        "no choices in: {err}"
    );
}

#[test]
fn open_engine_applies_server_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let (_engine, _log) = mock_engine(tmp.path(), "");
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(
        &paths.settings_file,
        format!(
            "[engine]\nprogram = \"{}\"\ncall_timeout = \"5s\"\n",
            tmp.path().join("engine").display()
        ),
    )
    .unwrap();

    assert_eq!(
        open_engine(&paths).unwrap().settings().call_timeout,
        Duration::from_secs(5),
        "open_engine did not apply call_timeout from settings.toml"
    );
}

#[test]
fn open_engine_fails_when_engine_binary_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    let no_engine = tmp.path().join("no-such-engine");
    crate::config::settings::save_key(
        &paths,
        "engine",
        "program",
        Some(&no_engine.display().to_string()),
    )
    .unwrap();

    let err = match open_engine(&paths) {
        Ok(_) => panic!("open_engine succeeded with a missing binary"),
        Err(err) => err,
    };
    assert!(
        err.downcast_ref::<EngineUnreachable>().is_some(),
        "expected EngineUnreachable, got: {err:#}"
    );
    assert!(
        !format!("{err:#}").contains("docker group"),
        "a missing binary is no socket permission problem: {err:#}"
    );
}

/// The deploy user outside the docker group gets EACCES on the socket. The
/// error says so and leaves the choice to the operator.
#[test]
fn a_socket_permission_error_names_both_choices() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    let program = socket_refusing_engine(tmp.path());
    crate::config::settings::save_key(
        &paths,
        "engine",
        "program",
        Some(&program.display().to_string()),
    )
    .unwrap();

    let err = match open_engine(&paths) {
        Ok(_) => panic!("open_engine succeeded with the socket refused"),
        Err(err) => err,
    };

    let text = format!("{err:#}");
    assert!(err.downcast_ref::<EngineUnreachable>().is_some(), "{text}");
    for wanted in [
        "cannot open the engine's socket",
        "docker group",
        "root-equivalent",
        "rootless podman",
        "riku changes neither",
    ] {
        assert!(text.contains(wanted), "missing {wanted:?} in: {text}");
    }
}
