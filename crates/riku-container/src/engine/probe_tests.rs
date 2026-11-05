use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use crate::engine::detect::EngineBinary;
use crate::engine::fake::fake_engine;
use crate::engine::probe::{answers_version, first_unanswered};
use crate::engine::Engine;

/// Limit for fake engines that exit at once. Only the hung program test needs
/// a short limit. Under a loaded parallel test run a freshly written `exit 0`
/// script has taken over 300 ms to finish, which failed the old 300 ms limit.
const ANSWER_LIMIT: Duration = Duration::from_secs(10);

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
    let bin = binary(
        dir.path(),
        "case \"$1\" in stats) echo 'unknown command' >&2; exit 1;; esac; exit 0",
    );

    let err = Engine::open(bin, Vec::new()).unwrap_err().to_string();

    assert!(err.contains("does not answer 'stats'"), "{err}");
    assert!(err.contains("riku needs: pull, build, run"), "{err}");
    assert!(err.contains("unknown command"), "stderr missing: {err}");
}

#[test]
fn probe_stops_a_command_that_never_answers() {
    let dir = tempfile::tempdir().unwrap();
    let bin = binary(dir.path(), "case \"$1\" in ps) exec sleep 30;; esac");

    let (command, reason) =
        first_unanswered(&bin.program, &[], std::time::Duration::from_millis(1500)).unwrap();

    assert_eq!(command, "ps", "{reason}");
    assert!(
        reason.contains("1.5s"),
        "reason should name the limit: {reason}"
    );
}

#[test]
fn probe_waits_for_the_slowest_command_not_the_sum() {
    let dir = tempfile::tempdir().unwrap();
    let bin = binary(dir.path(), "sleep 0.5");
    let started = Instant::now();

    let unanswered = first_unanswered(&bin.program, &[], ANSWER_LIMIT);

    assert_eq!(unanswered, None);
    // One after another the sixteen probes take at least 8 s.
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "probes ran one after another: {:?}",
        started.elapsed()
    );
}

#[test]
fn probe_reports_the_first_failing_command_in_required_order() {
    let dir = tempfile::tempdir().unwrap();
    let bin = binary(
        dir.path(),
        "case \"$1\" in wait) exit 1;; build) sleep 0.3; exit 1;; esac",
    );

    let (command, _) = first_unanswered(&bin.program, &[], ANSWER_LIMIT).unwrap();

    assert_eq!(command, "build", "a later command finished failing first");
}

#[test]
fn open_refuses_engine_without_login_or_image() {
    for command in ["login", "image"] {
        let dir = tempfile::tempdir().unwrap();
        let script = format!("case \"$1\" in {command}) echo 'unknown' >&2; exit 1;; esac");
        let bin = binary(dir.path(), &script);

        let err = Engine::open(bin, Vec::new()).unwrap_err().to_string();

        assert!(
            err.contains(&format!("does not answer '{command}'")),
            "{err}"
        );
    }
}

#[test]
fn open_reports_spawn_failure_instead_of_blaming_a_command() {
    let dir = tempfile::tempdir().unwrap();
    let bin = binary(dir.path(), "exit 0");
    std::fs::set_permissions(&bin.program, std::fs::Permissions::from_mode(0o644)).unwrap();

    let err = Engine::open(bin, Vec::new()).unwrap_err().to_string();

    assert!(err.contains("Permission denied"), "{err}");
}

#[test]
fn open_reads_identity_after_probe() {
    let dir = tempfile::tempdir().unwrap();
    let script = format!("case \"$2\" in --help) exit 0;; esac; echo '{PODMAN_VERSION}'");
    let bin = binary(dir.path(), &script);

    let engine = Engine::open(bin, Vec::new()).unwrap();

    assert_eq!(engine.identity().version, "5.2.2");
    assert_eq!(engine.identity().name, "podman");
}

#[test]
fn output_reports_command_and_stderr_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let script = format!(
        "case \"$2\" in --help) exit 0;; esac; case \"$1\" in version) echo '{PODMAN_VERSION}';; *) echo ' no such thing ' >&2; exit 3;; esac"
    );
    let engine = Engine::open(binary(dir.path(), &script), Vec::new()).unwrap();

    let err = engine
        .output(&["rm".into(), "gone".into()])
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("'rm'") && err.contains("no such thing"),
        "{err}"
    );
}

#[test]
fn answers_version_stops_hung_program_and_returns_false() {
    let dir = tempfile::tempdir().unwrap();
    let program = fake_engine(dir.path(), "docker", "exec sleep 30");
    let started = Instant::now();

    let answered = answers_version(&program, &[], Duration::from_millis(300));

    assert!(!answered, "sleeping program should return false");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "answers_version was not stopped within 10 s"
    );
}

#[test]
fn answers_version_accepts_zero_exit() {
    let dir = tempfile::tempdir().unwrap();
    let program = fake_engine(dir.path(), "docker", "exit 0");

    assert!(
        answers_version(&program, &[], ANSWER_LIMIT),
        "program exiting 0 should return true"
    );
}

#[test]
fn answers_version_rejects_nonzero_exit() {
    let dir = tempfile::tempdir().unwrap();
    let program = fake_engine(dir.path(), "docker", "exit 1");

    assert!(
        !answers_version(&program, &[], ANSWER_LIMIT),
        "program exiting 1 should return false"
    );
}
