//! Swapping a symlink's target in one step.
//!
//! Security model: as for [`super::atomic`], the directory may hold a name
//! someone planted. The temp link takes a fresh name per call from the same
//! process wide counter, and `symlink(2)` never replaces an existing entry,
//! so a taken name is skipped, never removed, and two threads swapping one
//! link never touch each other's temp link.

use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::atomic::{next_temp_seq, temp_path, TEMP_ATTEMPTS};

/// Point `link` at `target` by creating a temp link beside it and renaming
/// it over `link`, so `link` always resolves to the old or the new target,
/// never to nothing. The supervisor watches `workers-enabled`, and a moment
/// without the link would read as the worker being removed.
pub fn replace_symlink(target: &Path, link: &Path) -> Result<()> {
    for _ in 0..TEMP_ATTEMPTS {
        let tmp_link = temp_path(link, next_temp_seq());
        match std::os::unix::fs::symlink(target, &tmp_link) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(anyhow!("cannot link {}: {e}", tmp_link.display())),
        }
        if let Err(e) = std::fs::rename(&tmp_link, link) {
            let _ = std::fs::remove_file(&tmp_link);
            return Err(anyhow!("cannot replace {}: {e}", link.display()));
        }
        return Ok(());
    }
    Err(anyhow!(
        "no free temp link name beside {} after {TEMP_ATTEMPTS} tries",
        link.display()
    ))
}
