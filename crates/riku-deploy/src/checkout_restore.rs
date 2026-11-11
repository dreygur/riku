//! Putting the app checkout back at the last good release after a failed
//! deploy.
//!
//! A push resets `apps/<app>` to the pushed commit before it builds. When
//! the build fails the old processes keep running from the old build, but
//! the checkout would stay at the broken commit, and anything that later
//! reads it (a restart from an older riku, `riku run`, a plain
//! `riku deploy`) would act on code that never went live.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use anyhow::Result;

use crate::config::RikuPaths;
use crate::git_ops;
use crate::releases::ReleaseLog;

/// Reset the checkout of `app` to its last recorded release. Returns the
/// sha it was reset to, or `None` when there is no release yet (a failed
/// first deploy) or the checkout is already there.
pub(crate) fn restore_last_release(app: &str, paths: &RikuPaths) -> Result<Option<String>> {
    let released = match ReleaseLog::new(paths).list(app).pop() {
        Some(release) => release.sha,
        None => return Ok(None),
    };
    let checkout = paths.app_root.join(app);
    if git_ops::head_sha(&checkout).as_deref() == Some(released.as_str()) {
        return Ok(None);
    }
    if let Err(e) = git_ops::git_reset(&checkout, &released) {
        return Err(e);
    }
    git_ops::git_update_submodules(&checkout);
    Ok(Some(released))
}

#[cfg(test)]
#[path = "checkout_restore_tests.rs"]
mod tests;
