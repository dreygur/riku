//! Writing a plugin file so nothing ever runs a partial or temporary copy.
//!
//! Security: riku executes plugin files. A file is written to a hidden temp
//! file unique to this writer (`.<name>.<pid>.<serial>.new`, created new so
//! an existing file or symlink there is refused) with mode `0600`, synced,
//! given its final mode, then renamed over `<name>`. An interrupted write leaves at most a hidden, non executable
//! file, and runtime discovery skips hidden files.

use std::fs::{self, OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Result};

const TEMP_MODE: u32 = 0o600;

fn write_temp(tmp: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
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

/// A temp name no other writer uses: the CLI and the supervisor's install
/// route can write the same plugin at once, from separate processes or
/// threads.
fn temp_name(name: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    format!(".{name}.{}.{serial}.new", std::process::id())
}

/// Write `bytes` as `dir/<name>` with `mode`. `name` is a plain file name;
/// `dir` must exist. On error no temporary file is left behind.
pub(crate) fn write_plugin_file(dir: &Path, name: &str, bytes: &[u8], mode: u32) -> Result<()> {
    let tmp = dir.join(temp_name(name));
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
