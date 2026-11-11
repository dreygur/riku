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
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env_remove("SSH_ORIGINAL_COMMAND")
        .output()
        .expect("git push runs")
}

/// The bare repository with the hook riku would install, which names
/// `/usr/local/bin/riku`, pointed at the binary under test instead.
fn repo_with_hook(root: &Path, app: &str) {
    let riku = env!("CARGO_BIN_EXE_riku");
    let repo = root.join("riku/repos").join(app);
    let init = Command::new("git")
        .args(["init", "--quiet", "--bare"])
        .arg(&repo)
        .output()
        .expect("git init runs");
    assert!(init.status.success(), "git init: {init:?}");
    let hook = repo.join("hooks/post-receive");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(
        &hook,
        format!(
            "#!/usr/bin/env bash\nset -e\nAPP=\"$(basename \"$(pwd)\" .git)\"\n\
             cat | \"{riku}\" git-hook \"$APP\" \"$(pwd)\"\n"
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
}

#[test]
fn a_push_whose_deploy_fails_exits_non_zero_and_shows_the_failure_once() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    let work = committed_app(tmp.path());
    repo_with_hook(tmp.path(), "failapp");

    // No router is set up under this RIKU_ROOT, so the deploy is refused
    // after git has already taken the commits.
    let out = push_to_riku(&work, tmp.path(), "failapp");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "the push must fail when its deploy fails, stderr:\n{stderr}"
    );
    let failure_lines: Vec<&str> = stderr.lines().filter(|l| l.contains("■")).collect();
    assert_eq!(
        failure_lines.len(),
        1,
        "the client must see why the push failed, once, stderr:\n{stderr}"
    );
    assert!(
        failure_lines[0].starts_with("remote:"),
        "the failure comes from the hook through git's sideband, stderr:\n{stderr}"
    );
}

#[test]
fn a_first_push_prints_nothing_on_the_rail_before_the_deploy_intro() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    let work = committed_app(tmp.path());
    repo_with_hook(tmp.path(), "firstapp");

    let out = push_to_riku(&work, tmp.path(), "firstapp");
    let stderr = String::from_utf8_lossy(&out.stderr);

    let intro = stderr.lines().position(|l| l.contains("┌"));
    let Some(intro) = intro else {
        panic!("the push must show the deploy intro, stderr:\n{stderr}");
    };
    let before: Vec<&str> = stderr
        .lines()
        .take(intro)
        .filter(|l| l.contains("│"))
        .collect();
    assert!(
        before.is_empty(),
        "nothing may sit on the rail above the intro, got {before:?}, stderr:\n{stderr}"
    );
}
