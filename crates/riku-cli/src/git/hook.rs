//! Git post-receive hook handler: triggers deployment on push.

use anyhow::{anyhow, Result};
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::display;

/// Whether `app_path` still needs its first clone. Only a missing checkout
/// counts: an app without a Procfile is still a checkout, and cloning into
/// it again fails on the non-empty directory.
fn needs_clone(app_path: &Path) -> bool {
    !app_path.join(".git").exists()
}

fn clone_app(paths: &RikuPaths, app: &str) -> Result<()> {
    let repo_path = paths.git_root.join(app);
    let app_path = paths.app_root.join(app);
    let data_path = paths.data_root.join(app);

    for dir in [&app_path, &data_path] {
        if let Err(e) = fs::create_dir_all(dir) {
            return Err(anyhow!("failed to create {}: {e}", dir.display()));
        }
    }
    let status = Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg(&repo_path)
        .arg(app)
        .current_dir(&paths.app_root)
        .status();
    // B6: abort this ref's deploy when the clone fails. Continuing would
    // run do_deploy against an empty/half-created app directory.
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(anyhow!(
            "git clone failed for app '{}' from repo '{}'",
            app,
            repo_path.display()
        )),
        Err(e) => Err(anyhow!("failed to run git clone for app '{app}': {e}")),
    }
}

/// What one pushed ref does.
#[derive(Debug, PartialEq, Eq)]
enum RefPush {
    Deploy,
    /// Stored but not deployed, with the reason shown to the pusher.
    Skip(String),
}

const ZERO_SHA_CHAR: char = '0';
const BRANCH_PREFIX: &str = "refs/heads/";

fn git_in(repo: &Path, args: &[&str]) -> Result<std::process::Output> {
    match Command::new("git")
        .arg("--git-dir")
        .arg(repo)
        .args(args)
        .output()
    {
        Ok(output) => Ok(output),
        Err(e) => Err(anyhow!("failed to run git in {}: {e}", repo.display())),
    }
}

/// The repo's default branch ref: its `HEAD`. When `HEAD` names a branch
/// that does not exist (a repo created with `master` that only ever got
/// `main`), the pushed branch `refname` becomes `HEAD`, so the first branch
/// pushed is the one that deploys.
#[allow(clippy::question_mark)]
fn default_ref(repo: &Path, refname: &str) -> Result<String> {
    let head = match git_in(repo, &["symbolic-ref", "-q", "HEAD"]) {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(_) => return Err(anyhow!("{} has no symbolic HEAD", repo.display())),
        Err(e) => return Err(e),
    };
    let exists = match git_in(repo, &["rev-parse", "--verify", "-q", &head]) {
        Ok(output) => output.status.success(),
        Err(e) => return Err(e),
    };
    if exists || !refname.starts_with(BRANCH_PREFIX) {
        return Ok(head);
    }
    match git_in(repo, &["symbolic-ref", "HEAD", refname]) {
        Ok(output) if output.status.success() => Ok(refname.to_string()),
        Ok(_) => Err(anyhow!(
            "cannot point HEAD of {} at {refname}",
            repo.display()
        )),
        Err(e) => Err(e),
    }
}

/// Whether a pushed ref deploys: only the default branch does. Tags, other
/// branches and deletions are kept in the repo but not deployed.
fn ref_push(refname: &str, newrev: &str, default: &str) -> RefPush {
    if newrev.chars().all(|c| c == ZERO_SHA_CHAR) {
        return RefPush::Skip(format!("{refname} was deleted"));
    }
    if refname != default {
        let branch = default.strip_prefix(BRANCH_PREFIX).unwrap_or(default);
        return RefPush::Skip(format!(
            "{refname} is not the deploy branch; push to {branch} to deploy"
        ));
    }
    RefPush::Deploy
}

/// The bare repo of `app`: the hook passes its own path; otherwise the
/// standard location.
fn repo_dir(paths: &RikuPaths, app: &str, repo_path: Option<&str>) -> std::path::PathBuf {
    match repo_path {
        Some(path) => std::path::PathBuf::from(path),
        None => paths.git_root.join(format!("{app}.git")),
    }
}

/// Why a hook run failed.
enum HookFailure {
    /// The deploy failed and already printed its failure block.
    Shown(anyhow::Error),
    /// Failed before a deploy could report anything.
    Silent(anyhow::Error),
}

impl From<anyhow::Error> for HookFailure {
    fn from(e: anyhow::Error) -> Self {
        HookFailure::Silent(e)
    }
}

/// Post-receive git hook handler. A failure is also recorded for the push
/// that ran the hook, so `git push` exits non-zero (see `push_status`). A
/// deploy failure the deploy already printed exits 1 without printing it a
/// second time; git ignores a post-receive status anyway.
pub fn cmd_git_hook(paths: &RikuPaths, app: &str, repo_path: Option<&str>) -> Result<()> {
    match deploy_pushed_refs(paths, app, repo_path) {
        Ok(()) => Ok(()),
        Err(HookFailure::Shown(e)) => {
            super::push_status::record_deploy_failure(&e);
            std::process::exit(1);
        }
        Err(HookFailure::Silent(e)) => {
            super::push_status::record_deploy_failure(&e);
            Err(e)
        }
    }
}

#[allow(clippy::question_mark)]
fn deploy_pushed_refs(
    paths: &RikuPaths,
    app: &str,
    repo_path: Option<&str>,
) -> std::result::Result<(), HookFailure> {
    let app = match crate::util::validate_app_name(app) {
        Ok(app) => app,
        Err(e) => return Err(e.into()),
    };

    if let Some(actual_repo) = repo_path {
        if let Err(e) = link_external_repo(paths, &app, actual_repo) {
            return Err(e.into());
        }
    }

    let app_path = paths.app_root.join(&app);
    let repo = repo_dir(paths, &app, repo_path);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => return Err(anyhow!("failed to read pushed refs: {e}").into()),
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let newrev = parts[1];
        let refname = parts[2];

        let default = match default_ref(&repo, refname) {
            Ok(default) => default,
            Err(e) => return Err(e.into()),
        };
        if let RefPush::Skip(reason) = ref_push(refname, newrev, &default) {
            display::warn(&format!("Not deploying: {reason}"));
            continue;
        }

        if needs_clone(&app_path) {
            if let Err(e) = clone_app(paths, &app) {
                return Err(e.into());
            }
        }

        let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        if let Err(e) = crate::deploy::do_deploy(&app, paths, &deltas, Some(newrev)) {
            return Err(HookFailure::Shown(e));
        }
    }

    Ok(())
}

/// S4: Validate the symlink source before linking `~/.riku/repos/{app}.git` to it.
///
/// `repo_path` reaches us via the post-receive hook as `pwd` of the bare repo,
/// but `git-hook` is a CLI subcommand any local/SSH caller can invoke with an
/// arbitrary path, so the target is influenceable and must be validated.
///
/// Legitimate sources can legitimately live OUTSIDE `git_root`: `ensure_repo_symlink`
/// in `repo.rs` lets a user keep their bare repo at `~/{app}.git` and symlinks it
/// into `git_root`. We therefore cannot require the target to stay under `git_root`
/// without breaking that flow. Instead we apply the tightest checks that don't:
///   1. reject raw `..` traversal sequences in the supplied path,
///   2. canonicalize the target (rejects dangling/non-existent paths),
///   3. require the resolved target to be a real directory (a bare repo dir),
///   4. require the resolved basename to be `{app}` or `{app}.git` (the two repo
///      naming conventions used by `receive_pack.rs` and `repo.rs`), binding the
///      link to its app so a caller cannot point `{app}.git` at an unrelated repo,
///   5. refuse to overwrite an existing riku target that is NOT already a symlink,
///      so a real directory/file at the destination is never clobbered.
fn link_external_repo(paths: &RikuPaths, app: &str, actual_repo: &str) -> Result<()> {
    if actual_repo.contains("..") {
        return Err(anyhow::anyhow!(
            "Refusing repo path '{}' for app '{}': contains path traversal sequence",
            actual_repo,
            app
        ));
    }

    let actual_path = Path::new(actual_repo);
    if !actual_path.exists() {
        // Source absent: nothing to link, deploy proceeds against git_root/{app}.
        return Ok(());
    }

    let resolved = fs::canonicalize(actual_path).map_err(|e| {
        anyhow::anyhow!(
            "Cannot resolve repo path '{}' for app '{}': {}",
            actual_repo,
            app,
            e
        )
    })?;

    if !resolved.is_dir() {
        return Err(anyhow::anyhow!(
            "Refusing repo path '{}' for app '{}': resolved target is not a directory",
            resolved.display(),
            app
        ));
    }

    let expected_name = format!("{}.git", app);
    let resolved_name = resolved.file_name().and_then(|n| n.to_str());
    if resolved_name != Some(app) && resolved_name != Some(expected_name.as_str()) {
        return Err(anyhow::anyhow!(
            "Refusing repo path '{}' for app '{}': basename must be '{}' or '{}'",
            resolved.display(),
            app,
            app,
            expected_name
        ));
    }

    let riku_repo = paths.git_root.join(&expected_name);

    // If the pushed repo is already AT the canonical location (the common case:
    // a bare repo created directly under repos/<app>.git), it is correctly
    // placed and there is nothing to symlink. Without this, the guard below
    // would refuse it for being a real directory rather than a symlink.
    if riku_repo.canonicalize().ok().as_deref() == Some(resolved.as_path()) {
        return Ok(());
    }

    // Use symlink_metadata so we detect (and never clobber) a real dir/file even
    // when a dangling symlink is present.
    if let Ok(meta) = riku_repo.symlink_metadata() {
        if !meta.file_type().is_symlink() {
            return Err(anyhow::anyhow!(
                "Refusing to replace '{}' for app '{}': destination exists and is not a symlink",
                riku_repo.display(),
                app
            ));
        }
        // An existing symlink at the canonical location is already wired up.
        return Ok(());
    }

    std::os::unix::fs::symlink(&resolved, &riku_repo)?;
    tracing::debug!("linked {} to {}", riku_repo.display(), resolved.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn paths_for(root: &Path) -> RikuPaths {
        let git_root = root.join("repos");
        fs::create_dir_all(&git_root).unwrap();
        RikuPaths::from_dirs(root.to_path_buf(), Path::new("/home/test"))
    }

    #[test]
    fn clones_only_when_the_app_dir_is_not_a_checkout() {
        let tmp = TempDir::new().unwrap();
        let app_path = tmp.path().join("myapp");
        assert!(needs_clone(&app_path), "a missing app dir needs a clone");

        fs::create_dir_all(app_path.join(".git")).unwrap();
        assert!(
            !needs_clone(&app_path),
            "a checkout without a Procfile must not be cloned again"
        );
    }

    const SHA: &str = "4b17a4c0e1c2d3f4a5b6c7d8e9f0a1b2c3d4e5f6";
    const ZERO: &str = "0000000000000000000000000000000000000000";

    #[test]
    fn only_the_default_branch_deploys() {
        let main = "refs/heads/main";
        assert_eq!(ref_push(main, SHA, main), RefPush::Deploy);
        assert!(matches!(
            ref_push("refs/heads/feature", SHA, main),
            RefPush::Skip(reason) if reason.contains("push to main")
        ));
        assert!(matches!(
            ref_push("refs/tags/v1", SHA, main),
            RefPush::Skip(_)
        ));
        assert!(matches!(ref_push(main, ZERO, main), RefPush::Skip(_)));
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn first_pushed_branch_becomes_the_default_when_head_is_unborn() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("myapp.git");
        let work = tmp.path().join("work");
        fs::create_dir_all(&work).unwrap();
        git(
            tmp.path(),
            &["init", "--quiet", "--bare", repo.to_str().unwrap()],
        );
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/master"]);
        git(&work, &["init", "--quiet"]);
        git(
            &work,
            &["commit", "--quiet", "--allow-empty", "-m", "first"],
        );
        git(
            &work,
            &[
                "push",
                "--quiet",
                repo.to_str().unwrap(),
                "HEAD:refs/heads/main",
            ],
        );

        assert_eq!(
            default_ref(&repo, "refs/heads/main").unwrap(),
            "refs/heads/main"
        );

        git(
            &work,
            &[
                "push",
                "--quiet",
                repo.to_str().unwrap(),
                "HEAD:refs/heads/feature",
            ],
        );
        assert_eq!(
            default_ref(&repo, "refs/heads/feature").unwrap(),
            "refs/heads/main",
            "a later branch must not take over"
        );
    }

    #[test]
    fn rejects_traversal_sequence() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        let err = link_external_repo(&paths, "myapp", "/srv/../etc/myapp.git").unwrap_err();
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn absent_source_is_noop() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        let missing = tmp.path().join("does-not-exist");
        link_external_repo(&paths, "myapp", missing.to_str().unwrap()).unwrap();
        assert!(!paths.git_root.join("myapp.git").exists());
    }

    #[test]
    fn rejects_non_directory_source() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        let file = tmp.path().join("myapp.git");
        fs::write(&file, b"not a dir").unwrap();
        let err = link_external_repo(&paths, "myapp", file.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("not a directory"));
    }

    #[test]
    fn rejects_mismatched_basename() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        let other = tmp.path().join("evil.git");
        fs::create_dir_all(&other).unwrap();
        let err = link_external_repo(&paths, "myapp", other.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("basename must be"));
    }

    #[test]
    fn links_valid_dotgit_source() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        let src = tmp.path().join("myapp.git");
        fs::create_dir_all(&src).unwrap();
        link_external_repo(&paths, "myapp", src.to_str().unwrap()).unwrap();
        let dest = paths.git_root.join("myapp.git");
        assert!(dest.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(
            fs::read_link(&dest).unwrap(),
            fs::canonicalize(&src).unwrap()
        );
    }

    #[test]
    fn accepts_bare_app_basename() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        let src = tmp.path().join("myapp");
        fs::create_dir_all(&src).unwrap();
        link_external_repo(&paths, "myapp", src.to_str().unwrap()).unwrap();
        assert!(paths
            .git_root
            .join("myapp.git")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn refuses_to_clobber_real_destination_dir() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        let src = tmp.path().join("myapp.git");
        fs::create_dir_all(&src).unwrap();
        // A real directory already sits at the riku destination.
        fs::create_dir_all(paths.git_root.join("myapp.git")).unwrap();
        let err = link_external_repo(&paths, "myapp", src.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("not a symlink"));
    }
}
