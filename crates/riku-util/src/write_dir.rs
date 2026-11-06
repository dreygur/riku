//! The check every root write into a directory starts with.

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{anyhow, Result};

/// Root's uid, the owner a directory root writes into must have.
pub const ROOT_UID: u32 = 0;

/// Refuses `dir` for a root write unless it is a real directory owned by
/// `owner` that no group or other user can write. Otherwise another
/// account could swap the file root writes for a symlink between check and
/// rename. `owner` is [`ROOT_UID`] in production; tests pass their own uid.
pub fn check_write_dir(dir: &Path, owner: u32) -> Result<()> {
    let meta = match dir.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", dir.display())),
    };
    if !meta.is_dir() {
        return Err(anyhow!(
            "refusing to write into {}: not a directory (a symlink?)",
            dir.display()
        ));
    }
    if meta.uid() != owner {
        return Err(anyhow!(
            "refusing to write into {}: it is owned by uid {}, not {owner}",
            dir.display(),
            meta.uid()
        ));
    }
    if meta.mode() & 0o022 != 0 {
        return Err(anyhow!(
            "refusing to write into {}: it is group or world writable (mode {:o})",
            dir.display(),
            meta.mode() & 0o777
        ));
    }
    Ok(())
}
