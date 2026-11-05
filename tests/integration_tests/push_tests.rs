//! `git push` through `riku git-receive-pack`, the command an ssh push runs.
//!
//! The push goes over git's `ext::` transport, which runs the riku binary the
//! way sshd would and talks the pack protocol over its stdin and stdout.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .expect("git runs")
}

fn committed_app(root: &Path) -> std::path::PathBuf {
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "--quiet"]);
    fs::write(work.join("Procfile"), "web: true\n").unwrap();
    git(&work, &["add", "."]);
    let commit = git(&work, &["commit", "--quiet", "-m", "init"]);
    assert!(commit.status.success(), "commit: {commit:?}");
    work
}

fn push_to_riku(work: &Path, root: &Path, app: &str) -> Output {
    let riku = env!("CARGO_BIN_EXE_riku");
    Command::new("git")
        .args([
            "-c",
            "protocol.ext.allow=always",
            "push",
            &format!("ext::{riku} %S {app}"),
            "HEAD:refs/heads/main",
        ])
        .current_dir(work)
        .env("HOME", root.join("home"))
        .env("RIKU_ROOT", root.join("riku"))
        .env("RIKU_BIN", riku)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env_remove("SSH_ORIGINAL_COMMAND")
        .output()
        .expect("git push runs")
}

#[test]
fn a_push_whose_deploy_fails_exits_non_zero() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    let work = committed_app(tmp.path());

    // No router is set up under this RIKU_ROOT, so the deploy is refused
    // after git has already taken the commits.
    let out = push_to_riku(&work, tmp.path(), "failapp");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "the push must fail when its deploy fails, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("The push was received, but the deploy failed"),
        "the client must see why the push failed, stderr:\n{stderr}"
    );
}
