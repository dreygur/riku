//! Plugin execution helpers.

use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Check if a file is executable.
pub(crate) fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(path) {
            let mode = metadata.permissions().mode();
            return (mode & 0o100) != 0;
        }
        false
    }

    #[cfg(windows)]
    {
        path.extension()
            .map_or(false, |ext| ext == "exe" || ext == "bat" || ext == "cmd")
    }
}

/// Execute a client plugin with the given arguments.
///
/// Plugin interface:
/// - $1: server (e.g., "deploy@server.com")
/// - $2: app name
/// - $3: full command (including subcommands)
/// - $4+: additional arguments
pub(crate) fn execute_plugin(plugin_path: &Path, args: &[String]) -> Result<()> {
    let status = Command::new(plugin_path).args(args).status()?;

    if !status.success() {
        return Err(anyhow!(
            "Client plugin '{}' exited with code {}",
            plugin_path.display(),
            status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}
