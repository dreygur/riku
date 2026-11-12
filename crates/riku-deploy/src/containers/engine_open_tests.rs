use std::fs;
use std::time::Duration;

use super::*;
use crate::config::RikuPaths;
use crate::mock_engine::{mock_engine, socket_refusing_engine};

#[test]
fn open_engine_applies_server_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let (_engine, _log) = mock_engine(tmp.path(), "");
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(
        &paths.container_engine_file,
        tmp.path().join("engine").display().to_string(),
    )
    .unwrap();
    fs::write(&paths.settings_file, "[engine]\ncall_timeout = \"5s\"\n").unwrap();

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
    fs::write(
        &paths.container_engine_file,
        no_engine.display().to_string(),
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
    fs::write(&paths.container_engine_file, program.display().to_string()).unwrap();

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
