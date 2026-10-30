use crate::engine::detect::EngineBinary;
use crate::engine::fake::fake_engine;
use crate::engine::Engine;

const PODMAN_VERSION: &str = r#"{"Client":{"Version":"5.2.2"},"Server":{"Version":"5.2.2"}}"#;

fn binary(dir: &std::path::Path, script: &str) -> EngineBinary {
    EngineBinary {
        program: fake_engine(dir, "podman", script),
        binary_name: "podman".to_string(),
    }
}

#[test]
fn open_refuses_engine_missing_a_command() {
    let dir = tempfile::tempdir().unwrap();
    let bin = binary(dir.path(), "case \"$1\" in stats) exit 1;; esac; exit 0");

    let err = Engine::open(bin).unwrap_err().to_string();

    assert!(err.contains("does not answer 'stats'"), "{err}");
    assert!(err.contains("riku needs: pull, build, run"), "{err}");
}

#[test]
fn open_reads_identity_after_probe() {
    let dir = tempfile::tempdir().unwrap();
    let script = format!("case \"$2\" in --help) exit 0;; esac; echo '{PODMAN_VERSION}'");
    let bin = binary(dir.path(), &script);

    let engine = Engine::open(bin).unwrap();

    assert_eq!(engine.identity().version, "5.2.2");
    assert_eq!(engine.identity().name, "podman");
}

#[test]
fn output_reports_command_and_stderr_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let script = format!(
        "case \"$2\" in --help) exit 0;; esac; case \"$1\" in version) echo '{PODMAN_VERSION}';; *) echo ' no such thing ' >&2; exit 3;; esac"
    );
    let engine = Engine::open(binary(dir.path(), &script)).unwrap();

    let err = engine
        .output(&["rm".into(), "gone".into()])
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("'rm'") && err.contains("no such thing"),
        "{err}"
    );
}
