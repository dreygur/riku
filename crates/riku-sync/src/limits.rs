//! How much one sync may write on the server.
//!
//! The manifest is hostile, so its sizes are only claims; the receiver still
//! never writes more than they state (see [`crate::receive`]). These limits
//! refuse a sync whose claims are too large before anything is written: the
//! bytes the client is asked for are capped, and the whole new tree, which is
//! staged beside `live` (unchanged files are copied from it), must leave a
//! margin of free space on that filesystem.

use crate::wire::{EntryKind, Manifest};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Default for [`ReceiveLimits::max_needed_bytes`]: 10 GiB.
pub const DEFAULT_MAX_NEEDED_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// Default for [`ReceiveLimits::free_margin_bytes`]: 1 GiB.
pub const DEFAULT_FREE_MARGIN_BYTES: u64 = 1024 * 1024 * 1024;

/// Limits the server side of a sync enforces before it plans any writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiveLimits {
    /// Most bytes of file content one sync may ask the client to send, as
    /// declared by the manifest.
    pub max_needed_bytes: u64,
    /// Free space, in bytes, that must remain on the filesystem holding `live`
    /// once the new tree is staged.
    pub free_margin_bytes: u64,
}

impl Default for ReceiveLimits {
    fn default() -> ReceiveLimits {
        ReceiveLimits {
            max_needed_bytes: DEFAULT_MAX_NEEDED_BYTES,
            free_margin_bytes: DEFAULT_FREE_MARGIN_BYTES,
        }
    }
}

/// Declared size of every file in the manifest, which is what staging holds.
pub(crate) fn tree_bytes(manifest: &Manifest) -> u64 {
    manifest
        .entries
        .iter()
        .fold(0u64, |total, entry| match &entry.kind {
            EntryKind::File { size, .. } => total.saturating_add(*size),
            EntryKind::Dir | EntryKind::Symlink { .. } => total,
        })
}

/// Bytes an unprivileged process may still write on the filesystem holding
/// `dir`.
pub(crate) fn available_bytes(dir: &Path) -> anyhow::Result<u64> {
    let path = match CString::new(dir.as_os_str().as_bytes()) {
        Ok(path) => path,
        Err(e) => return Err(anyhow::anyhow!("{} holds a NUL: {e}", dir.display())),
    };
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is a valid NUL terminated string and `stat` is a writable
    // statvfs struct that outlives the call.
    let status = unsafe { libc::statvfs(path.as_ptr(), &mut stat) };
    if status != 0 {
        return Err(anyhow::anyhow!(
            "cannot read free space of {}: {}",
            dir.display(),
            std::io::Error::last_os_error()
        ));
    }
    // The field types differ by platform (u32 or u64), so the conversions are
    // needed on some and no-ops on others.
    #[allow(clippy::useless_conversion)]
    let free = u64::from(stat.f_bavail).saturating_mul(u64::from(stat.f_frsize));
    Ok(free)
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
