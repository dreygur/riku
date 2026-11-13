//! The one bare repo of an app, `repos/<app>.git`, and the check that the
//! hook runs for it.
//!
//! Security: `git-hook` is a CLI subcommand any local or SSH caller can run
//! with an arbitrary repo path, so the path is refused unless it resolves to
//! the app's own repo. The hook then reads only that repo, never the path it
//! was given.

use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::RikuPaths;
use crate::util::validation::REPO_SUFFIX;

/// The bare repo of `app`. A push to `riku@host:<app>` and to
/// `riku@host:<app>.git` both write it, and `riku apps create` makes it.
pub(crate) fn app_repo(paths: &RikuPaths, app: &str) -> PathBuf {
    paths.git_root.join(format!("{app}{REPO_SUFFIX}"))
}

/// Refuse a hook run for any repo but `app`'s own: a raw `..`, a path that
/// does not resolve, or one that resolves anywhere else than [`app_repo`]
/// directly inside `repos/`.
pub(super) fn check_hook_repo(paths: &RikuPaths, app: &str, hook_repo: &str) -> Result<()> {
    if hook_repo.contains("..") {
        return Err(anyhow!(
            "Refusing repo path '{hook_repo}' for app '{app}': contains path traversal sequence"
        ));
    }
    let resolved = match fs::canonicalize(Path::new(hook_repo)) {
        Ok(resolved) => resolved,
        Err(e) => {
            return Err(anyhow!(
                "Cannot resolve repo path '{hook_repo}' for app '{app}': {e}"
            ))
        }
    };
    let in_repos = fs::canonicalize(&paths.git_root)
        .is_ok_and(|git_root| resolved.parent() == Some(git_root.as_path()));
    let expected = app_repo(paths, app);
    match fs::canonicalize(&expected) {
        Ok(own) if in_repos && own == resolved => Ok(()),
        _ => Err(anyhow!(
            "Refusing repo path '{}' for app '{app}': the app's repo is {}",
            resolved.display(),
            expected.display()
        )),
    }
}

#[cfg(test)]
#[path = "hook_repo_tests.rs"]
mod tests;
