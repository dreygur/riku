//! Path checks shared by every read from the sender's tree and every write into
//! staging.
//!
//! Security model: a [`RelPath`] is already free of `..` and absolute forms, but
//! the directory it is joined onto may hold a symlink at any ancestor. These
//! checks use `symlink_metadata` on each ancestor, so a link is seen as a link
//! and never followed.

use crate::wire::RelPath;
use std::fs;
use std::path::{Path, PathBuf};

/// Fails unless every directory above `path` under `root` exists and is a real
/// directory, not a symlink.
pub(crate) fn ensure_real_dirs(root: &Path, path: &RelPath) -> anyhow::Result<()> {
    let text = path.as_str();
    for (index, _) in text.match_indices('/') {
        let ancestor = &text[..index];
        match fs::symlink_metadata(root.join(ancestor)) {
            Ok(meta) if meta.file_type().is_dir() => {}
            Ok(_) => {
                return Err(anyhow::anyhow!(
                    "{text} passes through {ancestor}, which is not a real directory"
                ))
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "cannot inspect {ancestor} on the way to {text}: {e}"
                ))
            }
        }
    }
    Ok(())
}

/// The full path of `path` under `root`, after checking that it is a regular
/// file reached through real directories only.
pub(crate) fn real_file(root: &Path, path: &RelPath) -> anyhow::Result<PathBuf> {
    if let Err(e) = ensure_real_dirs(root, path) {
        return Err(e);
    }
    let full = root.join(path.to_path());
    match fs::symlink_metadata(&full) {
        Ok(meta) if meta.file_type().is_file() => Ok(full),
        Ok(_) => Err(anyhow::anyhow!("{} is not a regular file", path.as_str())),
        Err(e) => Err(anyhow::anyhow!("cannot inspect {}: {e}", path.as_str())),
    }
}
