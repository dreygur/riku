//! Git post-receive hook handler — triggers deployment on push.

use anyhow::Result;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

/// Post-receive git hook handler.
pub fn cmd_git_hook(paths: &RikuPaths, app: &str, repo_path: Option<&str>) -> Result<()> {
    let app = crate::util::validate_app_name(app)?;

    if let Some(actual_repo) = repo_path {
        link_external_repo(paths, &app, actual_repo)?;
    }

    let repo_path = paths.git_root.join(&app);
    let app_path = paths.app_root.join(&app);
    let data_path = paths.data_root.join(&app);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let _oldrev = parts[0];
        let newrev = parts[1];
        let _refname = parts[2];

        // Clone repo if app directory doesn't exist or is empty (no Procfile)
        if !app_path.exists() || !app_path.join("Procfile").exists() {
            echo(&format!("-----> Creating app '{}'", app), "green");
            fs::create_dir_all(&app_path)?;
            if !data_path.exists() {
                fs::create_dir_all(&data_path)?;
            }
            let status = Command::new("git")
                .arg("clone")
                .arg("--quiet")
                .arg(&repo_path)
                .arg(&app)
                .current_dir(&paths.app_root)
                .status()?;
            // B6: abort this ref's deploy when the clone fails. Continuing would
            // run do_deploy against an empty/half-created app directory.
            if !status.success() {
                return Err(anyhow::anyhow!(
                    "git clone failed for app '{}' from repo '{}'",
                    app,
                    repo_path.display()
                ));
            }
        }

        // Call the actual deploy function
        let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        crate::deploy::do_deploy(&app, paths, &deltas, Some(newrev))?;
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
    echo(
        &format!("Symlinked {} → {}", riku_repo.display(), resolved.display()),
        "green",
    );
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
