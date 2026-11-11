use std::fs;
use std::time::{Duration, Instant};

use super::*;

fn shell(script: &str) -> Vec<String> {
    vec!["sh".to_string(), "-c".to_string(), script.to_string()]
}

#[test]
fn run_returns_output_and_fails_on_nonzero_exit() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    let resp = cmd_agent_run(&paths, "shop", &shell("echo hi; echo oops >&2; exit 3"));

    let code = resp.error.as_ref().map(|e| e.code.as_str());
    assert_eq!(code, Some("COMMAND_FAILED"));
    let data = resp.data.expect("a failed run must still carry its output");
    assert_eq!(data["exit_code"], 3);
    assert_eq!(data["stdout"], "hi\n");
    assert_eq!(data["stderr"], "oops\n");
}

#[test]
fn run_caps_output_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_captured(
        &shell("head -c 70000 /dev/zero"),
        dir.path(),
        &HashMap::new(),
        Duration::from_secs(10),
    )
    .unwrap();

    assert_eq!(outcome.stdout.text.len(), OUTPUT_CAP);
    assert!(outcome.stdout.truncated, "cut output not marked truncated");
}

#[test]
fn run_kills_a_command_and_its_children_past_the_time_limit() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();

    let outcome = run_captured(
        &shell("sleep 30 & sleep 30"),
        dir.path(),
        &HashMap::new(),
        Duration::from_millis(300),
    )
    .unwrap();

    assert!(outcome.status.is_none(), "expected a timeout, got an exit");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "a background child kept the output pipe open for {:?}",
        started.elapsed()
    );
}
