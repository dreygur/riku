//! File system utilities: recursive copy, directory counting.

use anyhow::Result;
use std::fs;
use std::path::Path;

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
