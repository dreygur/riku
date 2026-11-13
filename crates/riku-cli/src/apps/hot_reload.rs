//! `riku restart --hot`, kept as a name for `riku restart`: both restart
//! the running processes in place without a build. A container app is
//! replaced one copy at a time behind the router; a Procfile worker with a
//! health check is replaced by a probed canary.

use anyhow::Result;

use crate::config::RikuPaths;

/// Same as [`super::cmd_restart`] with no service named.
pub fn cmd_hot_reload(paths: &RikuPaths, app: &str) -> Result<()> {
    super::cmd_restart(paths, app, None)
}
