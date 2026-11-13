use std::fs;
use std::time::{Duration, Instant};

use super::*;

fn shell(script: &str) -> Vec<String> {
    vec!["sh".to_string(), "-c".to_string(), script.to_string()]
}

fn shop() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    (tmp, paths)
}

/// Requests a token for `cmd`, then runs it with that token.
fn confirmed_run(tmp: &tempfile::TempDir, paths: &RikuPaths, cmd: &[String]) -> AgentResponse {
    let asked = cmd_agent_run(paths, tmp.path(), "shop", cmd, None);
    let token = asked
        .confirm_token
        .expect("run did not ask for confirmation");
    cmd_agent_run(paths, tmp.path(), "shop", cmd, Some(&token))
}

#[test]
fn run_needs_a_token_bound_to_the_exact_command() {
    let (tmp, paths) = shop();

    let asked = cmd_agent_run(&paths, tmp.path(), "shop", &shell("echo ok"), None);
    let token = asked
        .confirm_token
        .expect("run did not ask for confirmation");
    let swapped = cmd_agent_run(&paths, tmp.path(), "shop", &shell("rm -rf /"), Some(&token));

    let code = swapped.error.map(|e| e.code);
    assert_eq!(
        code.as_deref(),
        Some("INVALID_TOKEN"),
        "token spent on another command"
    );
}

#[test]
fn run_returns_output_and_fails_on_nonzero_exit() {
    let (tmp, paths) = shop();

    let resp = confirmed_run(&tmp, &paths, &shell("echo hi; echo oops >&2; exit 3"));

    let code = resp.error.as_ref().map(|e| e.code.as_str());
    assert_eq!(code, Some("COMMAND_FAILED"));
    let data = resp.data.expect("a failed run must still carry its output");
    assert_eq!(data["exit_code"], 3);
    assert_eq!(data["stdout"], "hi\n");
    assert_eq!(data["stderr"], "oops\n");
}

#[test]
fn run_gives_the_command_only_path_home_and_the_app_env() {
    let (tmp, paths) = shop();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    fs::write(paths.env_root.join("shop/LIVE_ENV"), "GREETING=hello\n").unwrap();

    let resp = confirmed_run(&tmp, &paths, &["env".to_string()]);

    let data = resp.data.expect("env produced no data");
    let stdout = data["stdout"].as_str().unwrap();
    assert!(stdout.contains("GREETING=hello"), "{stdout}");
    assert!(
        !stdout.contains("CARGO_MANIFEST_DIR"),
        "server environment leaked into the command:\n{stdout}"
    );
}

#[test]
fn run_returns_when_a_background_child_keeps_the_pipe_open() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();

    let outcome = run_captured(
        &shell("sleep 30 & echo done"),
        dir.path(),
        &HashMap::new(),
        Duration::from_secs(60),
    )
    .unwrap();

    assert!(
        started.elapsed() < Duration::from_secs(10),
        "waited {:?} for a background child",
        started.elapsed()
    );
    assert_eq!(outcome.stdout.text, "done\n");
}

#[test]
fn run_applies_the_open_files_limit_the_app_sets() {
    let dir = tempfile::tempdir().unwrap();
    let app_env = HashMap::from([("RIKU_MAX_OPEN_FILES".to_string(), "64".to_string())]);

    let outcome = run_captured(
        &shell("ulimit -n"),
        dir.path(),
        &app_env,
        Duration::from_secs(10),
    )
    .unwrap();

    assert_eq!(
        outcome.stdout.text, "64\n",
        "the command must see the app's RIKU_MAX_OPEN_FILES=64 as ulimit -n"
    );
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
