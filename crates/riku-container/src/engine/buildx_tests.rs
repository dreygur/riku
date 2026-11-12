use crate::engine::detect::EngineBinary;
use crate::engine::fake::fake_engine;
use crate::engine::Engine;

const PROBE: &str = "case \"$2\" in --help) exit 0;; esac\n\
     case \"$1\" in version) echo '{\"Client\":{\"Version\":\"1\"}}'; exit 0;; esac\n";

fn engine_named(dir: &std::path::Path, name: &str, buildx: &str) -> Engine {
    let script = format!("{PROBE}case \"$1\" in buildx) {buildx};; esac");
    let binary = EngineBinary {
        program: fake_engine(dir, name, &script),
        binary_name: name.to_string(),
    };
    Engine::open(binary).unwrap()
}

#[test]
fn docker_without_buildx_reports_it_missing() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_named(
        dir.path(),
        "docker",
        "echo 'unknown command: docker buildx' >&2; exit 1",
    );

    assert_eq!(engine.docker_has_buildx(), Some(false));
}

#[test]
fn docker_with_buildx_reports_it_present() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_named(
        dir.path(),
        "docker",
        "echo 'github.com/docker/buildx v0.20.0'; exit 0",
    );

    assert_eq!(engine.docker_has_buildx(), Some(true));
}

#[test]
fn another_engine_is_not_asked_about_buildx() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_named(dir.path(), "podman", "exit 1");

    assert_eq!(engine.docker_has_buildx(), None);
}
