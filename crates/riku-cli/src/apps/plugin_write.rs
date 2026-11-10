//! Writing a plugin file so nothing ever runs a partial or temporary copy.
//!
//! Security: riku executes plugin files. A file is written to `.<name>.new`
//! with mode `0600`, synced, given its final mode, then renamed over
//! `<name>`. An interrupted write leaves at most a hidden, non executable
//! file, and runtime discovery skips hidden files.

use std::fs::{self, OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use anyhow::{anyhow, Result};

const TEMP_MODE: u32 = 0o600;

fn write_temp(tmp: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let mut file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(TEMP_MODE)
        .open(tmp)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("cannot create {}: {e}", tmp.display())),
    };
    if let Err(e) = file.write_all(bytes) {
        return Err(anyhow!("cannot write {}: {e}", tmp.display()));
    }
    if let Err(e) = file.sync_all() {
        return Err(anyhow!("cannot sync {}: {e}", tmp.display()));
    }
    match fs::set_permissions(tmp, Permissions::from_mode(mode)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot set the mode of {}: {e}", tmp.display())),
    }
}

/// Write `bytes` as `dir/<name>` with `mode`. `name` is a plain file name;
/// `dir` must exist. On error no temporary file is left behind.
pub(crate) fn write_plugin_file(dir: &Path, name: &str, bytes: &[u8], mode: u32) -> Result<()> {
    let tmp = dir.join(format!(".{name}.new"));
    let target = dir.join(name);
    let written = write_temp(&tmp, bytes, mode).and_then(|()| match fs::rename(&tmp, &target) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot replace {}: {e}", target.display())),
    });
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
#[path = "plugin_write_tests.rs"]
mod tests;
