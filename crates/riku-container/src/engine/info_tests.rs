use std::fs;
use std::path::Path;

use super::*;
use crate::engine::detect::EngineBinary;
use crate::engine::fake::fake_engine;

// The three fixtures follow the upstream docs for `info --format json` and
// stay unverified until captures from a real host replace them.
const DOCKER_ROOTLESS: &str = include_str!("info_docker_rootless.json");
const DOCKER_ROOTFUL: &str = include_str!("info_docker_rootful.json");
const PODMAN_ROOTLESS: &str = include_str!("info_podman_rootless.json");

fn engine_reporting(dir: &Path, info_json: &str, env: Vec<(String, String)>) -> Engine {
    let fixture = dir.join("info.json");
    fs::write(&fixture, info_json).unwrap();
    let script = format!(
        "case \"$2\" in --help) exit 0;; esac\n\
         case \"$1\" in\n\
         version) echo '{{\"Client\":{{\"Version\":\"1\"}}}}';;\n\
         info) cat '{}';;\n\
         esac",
        fixture.display()
    );
    let binary = EngineBinary {
        program: fake_engine(dir, "engine", &script),
        binary_name: "engine".to_string(),
    };
    Engine::open(binary, env).unwrap()
}

#[test]
fn docker_rootless_shape_parses() {
    let rootless_dir = tempfile::tempdir().unwrap();
    let rootful_dir = tempfile::tempdir().unwrap();
    let rootless = engine_reporting(rootless_dir.path(), DOCKER_ROOTLESS, Vec::new());
    let rootful = engine_reporting(rootful_dir.path(), DOCKER_ROOTFUL, Vec::new());

    assert!(rootless.is_rootless().unwrap(), "name=rootless not found");
    assert!(!rootful.is_rootless().unwrap(), "rootful read as rootless");
}

#[test]
fn podman_rootless_shape_parses() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_reporting(dir.path(), PODMAN_ROOTLESS, Vec::new());

    assert!(
        engine.is_rootless().unwrap(),
        "host.security.rootless ignored"
    );
}

#[test]
fn info_without_either_field_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_reporting(dir.path(), r#"{"ServerVersion":"1"}"#, Vec::new());

    let err = engine.is_rootless().unwrap_err().to_string();

    assert!(err.contains("host.security.rootless"), "{err}");
}

#[test]
fn rootful_engine_refused_in_rootless_mode() {
    let dir = tempfile::tempdir().unwrap();
    let env = vec![(
        "DOCKER_HOST".to_string(),
        "unix:///var/run/docker.sock".to_string(),
    )];
    let rootful = engine_reporting(dir.path(), DOCKER_ROOTFUL, env);

    let err = rootful.require_rootless().unwrap_err().to_string();

    assert!(err.contains("mode is rootless"), "{err}");
    assert!(err.contains("unix:///var/run/docker.sock"), "{err}");
    assert!(err.contains("sudo riku init"), "{err}");
}

#[test]
fn rootless_engine_passes_the_mode_check() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_reporting(dir.path(), DOCKER_ROOTLESS, Vec::new());

    engine.require_rootless().unwrap();
}
