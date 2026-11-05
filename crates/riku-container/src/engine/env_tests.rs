use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use riku_config::settings::EngineMode;

use super::engine_env_under;
use crate::engine::detect::EngineBinary;
use crate::engine::fake::fake_engine;
use crate::engine::fixture::{logged, ID};
use crate::engine::{ContainerId, Engine, LogRange};

fn own_uid(dir: &Path) -> u32 {
    fs::metadata(dir).unwrap().uid()
}

fn runtime_dir(root: &Path, uid: u32, mode: u32) -> std::path::PathBuf {
    let dir = root.join(uid.to_string());
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).unwrap();
    dir
}

#[test]
fn runtime_dir_wrong_mode_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let uid = own_uid(tmp.path());
    let dir = runtime_dir(tmp.path(), uid, 0o755);

    let err = engine_env_under(tmp.path(), EngineMode::Rootless, None, uid)
        .unwrap_err()
        .to_string();

    assert!(
        err.contains(&dir.display().to_string()) && err.contains("0755"),
        "error should name the path and the mode: {err}"
    );
}

#[test]
fn runtime_dir_owned_by_another_uid_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let other = own_uid(tmp.path()) + 1;
    runtime_dir(tmp.path(), other, 0o700);

    let err = engine_env_under(tmp.path(), EngineMode::Rootless, None, other)
        .unwrap_err()
        .to_string();

    assert!(err.contains("is owned by uid"), "{err}");
}

#[test]
fn missing_runtime_dir_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let uid = own_uid(tmp.path());

    let err = engine_env_under(tmp.path(), EngineMode::Rootless, None, uid)
        .unwrap_err()
        .to_string();

    assert!(
        err.contains(&tmp.path().join(uid.to_string()).display().to_string()),
        "error should name the directory: {err}"
    );
}

#[test]
fn rootless_sets_runtime_dir_and_finds_the_session_bus() {
    let tmp = tempfile::tempdir().unwrap();
    let uid = own_uid(tmp.path());
    let dir = runtime_dir(tmp.path(), uid, 0o700);
    let runtime = ("XDG_RUNTIME_DIR".to_string(), dir.display().to_string());

    let without_bus = engine_env_under(tmp.path(), EngineMode::Rootless, None, uid).unwrap();
    fs::write(dir.join("bus"), "").unwrap();
    let with_bus = engine_env_under(tmp.path(), EngineMode::Rootless, None, uid).unwrap();

    assert_eq!(without_bus, std::slice::from_ref(&runtime));
    assert_eq!(
        with_bus,
        [
            runtime,
            (
                "DBUS_SESSION_BUS_ADDRESS".to_string(),
                format!("unix:path={}", dir.join("bus").display())
            )
        ]
    );
}

#[test]
fn caller_cannot_select_the_daemon() {
    let selectors = [
        "DOCKER_HOST",
        "DOCKER_CONTEXT",
        "CONTAINER_HOST",
        "CONTAINER_CONNECTION",
    ];
    let mut command = std::process::Command::new("engine");
    for name in selectors {
        command.env(name, "caller");
    }

    super::apply_env(&mut command, &[]);

    let left: Vec<_> = command.get_envs().filter(|(_, v)| v.is_some()).collect();
    assert!(left.is_empty(), "caller values survived: {left:?}");
}

#[test]
fn host_becomes_docker_host() {
    let tmp = tempfile::tempdir().unwrap();
    let uid = own_uid(tmp.path());

    let rootful = engine_env_under(
        tmp.path(),
        EngineMode::Rootful,
        Some("unix:///edge.sock"),
        uid,
    )
    .unwrap();
    let rootful_bare = engine_env_under(tmp.path(), EngineMode::Rootful, None, uid).unwrap();

    assert_eq!(
        rootful,
        [("DOCKER_HOST".to_string(), "unix:///edge.sock".to_string())]
    );
    assert!(rootful_bare.is_empty(), "{rootful_bare:?}");
}

#[test]
fn every_spawn_carries_env() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("env.log");
    let script = format!(
        "printf '%s %s %s\\n' \"$1\" \"${{XDG_RUNTIME_DIR-unset}}\" \"${{DOCKER_HOST-unset}}\" >> '{}'\n\
         case \"$2\" in --help) exit 0;; esac\n\
         case \"$1\" in\n\
           version) echo '{{\"Client\":{{\"Version\":\"1\"}}}}';;\n\
           login) cat > /dev/null;;\n\
         esac",
        log.display()
    );
    let binary = EngineBinary {
        program: fake_engine(dir.path(), "engine", &script),
        binary_name: "engine".to_string(),
    };
    let env = vec![
        ("XDG_RUNTIME_DIR".to_string(), "/run/user/4242".to_string()),
        ("DOCKER_HOST".to_string(), "unix:///edge.sock".to_string()),
    ];
    let engine = Engine::open(binary, env).unwrap();
    let id = ContainerId::try_from(ID).unwrap();

    engine.ping().unwrap();
    engine.pull("registry.example/myapp:1").unwrap();
    engine.login("registry.example", "deploy", "token").unwrap();
    assert!(engine.has_events());
    engine.timestamped_logs(&id, &LogRange::All, 1024).unwrap();
    let events = engine.events_command();
    let events_env: Vec<_> = events
        .get_envs()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();

    let lines = logged(&log);
    let commands: Vec<&str> = lines.iter().map(|l| l.split(' ').next().unwrap()).collect();
    for expected in ["ps", "version", "pull", "login", "events", "logs"] {
        assert!(
            commands.contains(&expected),
            "no '{expected}' call in {commands:?}"
        );
    }
    for line in &lines {
        assert!(
            line.ends_with(" /run/user/4242 unix:///edge.sock"),
            "engine call ran without the computed env: {line}"
        );
    }
    assert!(
        events_env.contains(&(
            "XDG_RUNTIME_DIR".to_string(),
            Some("/run/user/4242".to_string())
        )),
        "events command lacks the env: {events_env:?}"
    );
}
