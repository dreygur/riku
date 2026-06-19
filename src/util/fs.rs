//! File system utilities: recursive copy, directory counting, atomic writes.

use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Write `contents` to `path` atomically: write to a `.tmp-<pid>` sibling in
/// the same directory, `fsync` it, then `rename` over the destination.
///
/// A direct `fs::write` leaves a truncated/corrupt file if the process dies
/// (crash, OOM-kill) mid-write; a concurrent reader (e.g. the supervisor's
/// `notify::Watcher` picking up worker TOML / nginx conf changes) can then
/// observe that half-written state. `rename(2)` within the same filesystem
/// is atomic, so readers only ever see the old complete file or the new
/// complete file, never a partial one.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;

    let tmp_path = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id()
    ));

    let write_result = (|| -> Result<()> {
        let mut tmp_file = fs::File::create(&tmp_path)
            .with_context(|| format!("creating temp file {}", tmp_path.display()))?;
        tmp_file.write_all(contents)?;
        tmp_file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    fs::rename(&tmp_path, path)
        .with_context(|| format!("renaming {} to {}", tmp_path.display(), path.display()))?;
    Ok(())
}

/// Recursively copy `source` into `dest`, skipping `.git` and `node_modules`.
pub fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let src = entry.path();
        let dst = dest.join(entry.file_name());
        if src.is_dir() {
            if should_skip_dir(&src) {
                continue;
            }
            copy_dir_recursive(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Count all files (not directories) under `dir` recursively.
pub fn count_files(dir: &Path) -> Result<usize> {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files(&path)?;
            } else {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Returns true for directories that should never be copied into the app dir.
fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .map(|n| n == ".git" || n == "node_modules" || n == ".gitignore")
        .unwrap_or(false)
}
