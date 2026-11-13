//! `git push` through `riku git-receive-pack`, the command an ssh push runs.
//!
//! The push goes over git's `ext::` transport, which runs the riku binary the
//! way sshd would and talks the pack protocol over its stdin and stdout.

use std::fs;
use std::path::{Path, PathBuf};
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

fn committed_app(root: &Path) -> PathBuf {
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

/// The app's one repository, `repos/<app>.git`.
fn app_repo(root: &Path, app: &str) -> PathBuf {
    root.join("riku/repos").join(format!("{app}.git"))
}

/// The bare repository with the hook riku would install, which names
/// `/usr/local/bin/riku`, pointed at the binary under test instead.
fn repo_with_hook(root: &Path, app: &str) {
    let repo = app_repo(root, app);
    let init = Command::new("git")
        .args(["init", "--quiet", "--bare"])
        .arg(&repo)
        .output()
        .expect("git init runs");
    assert!(init.status.success(), "git init: {init:?}");
    hook_under_test(&repo);
}

fn hook_under_test(repo: &Path) {
    let riku = env!("CARGO_BIN_EXE_riku");
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

fn rev(dir: &Path, name: &str) -> String {
    let out = git(dir, &["rev-parse", "--verify", "-q", name]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The hook cloned the pushed commit into `apps/<app>` and ran the deploy.
/// No router is set up here, so the deploy itself is refused later on.
fn assert_deployed(root: &Path, app: &str, work: &Path, out: &Output) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let checkout = root.join("riku/apps").join(app);
    assert_eq!(
        rev(&checkout, "HEAD"),
        rev(work, "HEAD"),
        "the hook must clone the pushed commit into {}, stderr:\n{stderr}",
        checkout.display()
    );
    assert!(
        stderr.contains("┌"),
        "the hook must start the deploy, stderr:\n{stderr}"
    );
}

#[test]
fn a_first_push_to_app_dot_git_deploys() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    let work = committed_app(tmp.path());
    repo_with_hook(tmp.path(), "dotgit");

    let out = push_to_riku(&work, tmp.path(), "dotgit.git");

    assert_deployed(tmp.path(), "dotgit", &work, &out);
}

#[test]
fn apps_create_then_a_push_to_the_bare_name_deploys() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    let work = committed_app(tmp.path());
    let create = Command::new(env!("CARGO_BIN_EXE_riku"))
        .args(["apps", "create", "made"])
        .env("HOME", tmp.path().join("home"))
        .env("RIKU_ROOT", tmp.path().join("riku"))
        .env("XDG_CONFIG_HOME", tmp.path().join("config"))
        .output()
        .expect("riku runs");
    assert!(create.status.success(), "apps create: {create:?}");
    hook_under_test(&app_repo(tmp.path(), "made"));

    let out = push_to_riku(&work, tmp.path(), "made");

    assert_deployed(tmp.path(), "made", &work, &out);
}

#[test]
fn both_spellings_of_an_app_push_to_one_repository() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    let work = committed_app(tmp.path());
    repo_with_hook(tmp.path(), "twice");

    push_to_riku(&work, tmp.path(), "twice");
    git(
        &work,
        &["commit", "--quiet", "--allow-empty", "-m", "second"],
    );
    push_to_riku(&work, tmp.path(), "twice.git");

    // Both deploys fail here (no router), so the branch is rolled back; the
    // pushed commit must still be in the one repository.
    let head = rev(&work, "HEAD");
    let has_commit = git(
        &app_repo(tmp.path(), "twice"),
        &["cat-file", "-e", &format!("{head}^{{commit}}")],
    );
    assert!(
        has_commit.status.success(),
        "the second push must land in the repository the first one wrote"
    );
    assert!(
        !tmp.path().join("riku/repos/twice").exists(),
        "no second repository may appear for the bare name"
    );
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
fn a_failed_push_leaves_no_branch_so_pushing_again_retries_the_deploy() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    let work = committed_app(tmp.path());
    repo_with_hook(tmp.path(), "retry");

    // No router is set up, so the deploy fails after git took the branch.
    push_to_riku(&work, tmp.path(), "retry");

    assert_eq!(
        rev(&app_repo(tmp.path(), "retry"), "refs/heads/main"),
        "",
        "the failed first push left its branch on the server"
    );
    let again = push_to_riku(&work, tmp.path(), "retry");
    let stderr = String::from_utf8_lossy(&again.stderr);
    assert!(
        !stderr.contains("Everything up-to-date") && stderr.contains("┌"),
        "pushing the same commit again did not retry the deploy, stderr:\n{stderr}"
    );
}

/// A PATH holding a `git` that always fails and a `git-receive-pack` that
/// leaves a marker file, so a test can tell whether riku went on past it.
fn path_with_failing_git(root: &Path) -> PathBuf {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let marker = root.join("receive-pack-ran");
    let scripts = [
        ("git", "#!/bin/sh\nexit 1\n".to_string()),
        (
            "git-receive-pack",
            format!("#!/bin/sh\n: > {}\n", marker.display()),
        ),
    ];
    for (name, body) in scripts {
        let script = bin.join(name);
        fs::write(&script, body).unwrap();
        fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    }
    bin
}

fn riku_with_failing_git(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_riku"))
        .args(args)
        .env("PATH", path_with_failing_git(root))
        .env("HOME", root.join("home"))
        .env("RIKU_ROOT", root.join("riku"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env_remove("SSH_ORIGINAL_COMMAND")
        .output()
        .expect("riku runs")
}

#[test]
fn apps_create_fails_when_git_init_fails() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();

    let out = riku_with_failing_git(tmp.path(), &["apps", "create", "nogit"]);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "apps create must fail when git init fails, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("git init"),
        "the error must name git init, stderr:\n{stderr}"
    );
    assert!(
        !app_repo(tmp.path(), "nogit")
            .join("hooks/post-receive")
            .exists(),
        "no hook may be written into a repository git did not create"
    );
}

#[test]
fn a_push_stops_when_git_init_fails() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();

    let out = riku_with_failing_git(tmp.path(), &["git-receive-pack", "nogit"]);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "the push must fail when git init fails, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("git init"),
        "the error must name git init, stderr:\n{stderr}"
    );
    assert!(
        !tmp.path().join("receive-pack-ran").exists(),
        "git-receive-pack must not run after git init failed, stderr:\n{stderr}"
    );
}

/// The hook runs on every push as the deploy user. sshd sessions often get
/// umask 002 from pam_umask, and the hook riku makes on a first push must
/// still be 0755, not group writable. Nothing is pushed: receive-pack
/// lists the refs to the closed stdin and ends, so the hook never runs.
#[test]
fn the_hook_a_first_push_creates_is_0755_under_umask_002() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let out = Command::new("sh")
        .args(["-c", "umask 002 && exec \"$0\" git-receive-pack lazy"])
        .arg(env!("CARGO_BIN_EXE_riku"))
        .env("HOME", tmp.path().join("home"))
        .env("RIKU_ROOT", tmp.path().join("riku"))
        .env("XDG_CONFIG_HOME", tmp.path().join("config"))
        .env_remove("SSH_ORIGINAL_COMMAND")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("riku runs");

    let hook = app_repo(tmp.path(), "lazy").join("hooks/post-receive");
    let mode = match fs::metadata(&hook) {
        Ok(meta) => meta.permissions().mode() & 0o777,
        Err(e) => panic!("no hook at {} ({e}), riku: {out:?}", hook.display()),
    };
    assert_eq!(mode, 0o755, "hook mode {mode:o}");
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
