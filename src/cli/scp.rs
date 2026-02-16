use anyhow::Result;
use std::process::Command;

use crate::config::RikuPaths;

/// Simple wrapper to allow scp to work.
pub fn cmd_scp(paths: &RikuPaths, args: &[String]) -> Result<()> {
    let status = Command::new("scp")
        .args(args)
        .current_dir(&paths.git_root)
        .status()?;

    std::process::exit(status.code().unwrap_or(1));
}
