//! Mutual exclusion between syncs of one live directory.
//!
//! The lock is an exclusive `flock` on `<live>.sync-lock`, held for the whole
//! apply (recovery included) and released when the returned file is dropped, or
//! by the kernel if the process dies. The lock file itself is never deleted:
//! unlinking it would let a second process lock a new file while the first
//! still holds the old one.

use crate::swap::sibling;
use std::fs::{File, OpenOptions, TryLockError};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const LOCK_SUFFIX: &str = ".sync-lock";

/// Proof that the sync lock for one live directory is held. Only [`acquire`]
/// makes one; dropping it releases the lock.
pub(crate) struct SyncLock {
    _file: File,
}

/// The lock is held by another sync. Callers that need to tell this apart from
/// a real failure use `downcast_ref::<SyncBusy>()` on the error.
#[derive(Debug)]
pub(crate) struct SyncBusy {
    live: String,
}

impl std::fmt::Display for SyncBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "another sync of {} is running", self.live)
    }
}

impl std::error::Error for SyncBusy {}

/// Takes the lock without waiting. A second concurrent sync fails at once. The
/// lock file is opened with `O_NOFOLLOW`, so a symlink planted at its path is
/// refused instead of followed.
pub(crate) fn acquire(live: &Path) -> anyhow::Result<SyncLock> {
    let path = match sibling(live, LOCK_SUFFIX) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    let opened = OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path);
    let file = match opened {
        Ok(file) => file,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot open lock file {}: {e}",
                path.display()
            ))
        }
    };
    match file.try_lock() {
        Ok(()) => Ok(SyncLock { _file: file }),
        Err(TryLockError::WouldBlock) => Err(anyhow::Error::new(SyncBusy {
            live: live.display().to_string(),
        })),
        Err(TryLockError::Error(e)) => Err(anyhow::anyhow!("cannot lock {}: {e}", path.display())),
    }
}
