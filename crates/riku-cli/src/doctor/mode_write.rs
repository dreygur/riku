//! Writing files that may hold secrets without loosening their mode: a
//! replaced file keeps the mode it had (`0600` when new), and the new content
//! goes to a fresh file first, then replaces the old one by rename.

#![allow(clippy::question_mark)]

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

const NEW_FILE_MODE: u32 = 0o600;
const TEMP_SUFFIX: &str = ".new";
const BACKUP_SUFFIX: &str = ".bak";

fn mode_of(path: &Path) -> u32 {
    match fs::metadata(path) {
        Ok(meta) => meta.permissions().mode() & 0o777,
        Err(_) => NEW_FILE_MODE,
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Create `path`, which must not exist, with `mode` and `text`.
fn write_new(path: &Path, text: &str, mode: u32) -> Result<()> {
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("cannot create {}: {e}", path.display())),
    };
    match file.write_all(text.as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot write {}: {e}", path.display())),
    }
}

/// Write `text` to `path` by rename, with the mode `path` has now.
pub(super) fn replace_keeping_mode(path: &Path, text: &str) -> Result<()> {
    let tmp = with_suffix(path, TEMP_SUFFIX);
    let _ = fs::remove_file(&tmp);
    if let Err(e) = write_new(&tmp, text, mode_of(path)) {
        return Err(e);
    }
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot replace {}: {e}", path.display())),
    }
}

/// Save `text` as `<path>.bak` with `path`'s mode, unless a backup exists.
pub(super) fn back_up_once(path: &Path, text: &str) -> Result<()> {
    let backup = with_suffix(path, BACKUP_SUFFIX);
    if backup.exists() {
        return Ok(());
    }
    write_new(&backup, text, mode_of(path))
}
