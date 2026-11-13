//! The dashboard ends with the session that started it.
//!
//! `ssh riku@host dashboard ...` without a tty gets no SIGHUP when the
//! client goes away. Its stdin pipe closing is the sign the session ended.
//! A dashboard whose stdin is not a pipe (a systemd unit, `/dev/null`) has
//! no session to follow and keeps running.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn start(home: &std::path::Path, stdin: Stdio) -> Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_riku"))
        .args(["dashboard", "--local", "--bind", "127.0.0.1:0"])
        .env("HOME", home)
        .env("RIKU_ROOT", home.join(".riku"))
        .env("XDG_CONFIG_HOME", home)
        .env_remove("SSH_ORIGINAL_COMMAND")
        .env_remove("RIKU_DASHBOARD_TOKEN")
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let bound = BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .any(|line| line.starts_with("riku-dashboard-bound "));
    assert!(bound, "the dashboard never reported its port");
    child
}

/// Waits up to `limit` for `child` to exit; true when it did.
fn exits_within(child: &mut Child, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn dashboard_exits_when_its_stdin_pipe_closes() {
    let home = tempfile::tempdir().unwrap();
    let mut child = start(home.path(), Stdio::piped());

    drop(child.stdin.take());

    let exited = exits_within(&mut child, Duration::from_secs(10));
    if !exited {
        child.kill().unwrap();
    }
    assert!(exited, "the dashboard outlived its closed stdin");
}

#[test]
fn dashboard_with_no_session_on_stdin_keeps_running() {
    let home = tempfile::tempdir().unwrap();
    let mut child = start(home.path(), Stdio::null());

    let exited = exits_within(&mut child, Duration::from_millis(1500));
    child.kill().unwrap();
    child.wait().unwrap();

    assert!(
        !exited,
        "a dashboard with /dev/null on stdin must keep serving"
    );
}
