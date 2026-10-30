//! Mutual exclusion between syncs of one live directory.
//!
//! The lock is an exclusive `flock` on `<live>.sync-lock`, held for the whole
//! apply (recovery included) and released when the returned file is dropped, or
//! by the kernel if the process dies. The lock file itself is never deleted:
//! unlinking it would let a second process lock a new file while the first
//! still holds the old one.

use crate::swap::sibling;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

const LOCK_SUFFIX: &str = ".sync-lock";

/// Takes the lock without waiting. A second concurrent sync fails at once.
pub(crate) fn acquire(live: &Path) -> anyhow::Result<File> {
    let path = match sibling(live, LOCK_SUFFIX) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    let file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => file,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot open lock file {}: {e}",
                path.display()
            ))
        }
    };
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(anyhow::anyhow!(
            "another sync of {} is running",
            live.display()
        )),
        Err(TryLockError::Error(e)) => Err(anyhow::anyhow!("cannot lock {}: {e}", path.display())),
    }
}
