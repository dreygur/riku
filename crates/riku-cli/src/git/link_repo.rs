//! Linking a bare repo kept outside `git_root` into it, for the hook.

use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;

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
pub(super) fn link_external_repo(paths: &RikuPaths, app: &str, actual_repo: &str) -> Result<()> {
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
#[path = "link_repo_tests.rs"]
mod tests;
