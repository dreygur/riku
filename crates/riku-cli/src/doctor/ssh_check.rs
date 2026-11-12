//! `riku doctor` check that a riku deploy key is authorized for `git push`.
//!
//! Security: under `sudo riku doctor` this reads the deploy user's
//! `authorized_keys` as root. Root refuses a `.ssh` that is not a real
//! directory (a symlink to another user's directory, say). The file is
//! opened with `O_NOFOLLOW` and `O_NONBLOCK` and must be a regular file, so a
//! symlink or FIFO planted at that name is refused instead of followed or
//! waited on, and at most [`MAX_LEN`] bytes are read. Only a count of riku
//! lines leaves this module.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use nix::fcntl::OFlag;

use crate::util::authorized_keys::{authorized_keys_path, RIKU_KEY_MARKER};

use super::Check;

const NAME: &str = "ssh access";

/// The most of `authorized_keys` read; a bigger file is refused.
const MAX_LEN: u64 = 1024 * 1024;

/// Who reads the keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    /// Root, reading the deploy user's files.
    Root,
    /// The user whose home it is.
    Owner,
}

impl Reader {
    pub fn current() -> Reader {
        if nix::unistd::Uid::effective().is_root() {
            Reader::Root
        } else {
            Reader::Owner
        }
    }
}

/// The number of riku deploy keys in the `authorized_keys` of the user whose
/// home is `home`. A missing file is a `NotFound` error.
pub fn riku_key_count(home: &Path, reader: Reader) -> io::Result<usize> {
    let path = authorized_keys_path(home);
    if reader == Reader::Root {
        if let Some(ssh_dir) = path.parent() {
            if let Err(e) = real_directory(ssh_dir) {
                return Err(e);
            }
        }
    }
    let text = match read_regular_file(&path) {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    Ok(text
        .lines()
        .filter(|line| line.contains(RIKU_KEY_MARKER))
        .count())
}

/// Count the riku deploy keys in the `authorized_keys` of the user whose
/// home is `home`.
pub fn ssh_access(home: &Path, reader: Reader) -> Check {
    let path = authorized_keys_path(home);
    let keys = match riku_key_count(home, reader) {
        Ok(keys) => keys,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Check::warn(
                NAME,
                "no authorized_keys: git push deploys need a key (riku init)",
            )
        }
        Err(e) => return Check::warn(NAME, format!("could not read {}: {e}", path.display())),
    };
    if keys > 0 {
        Check::ok(NAME, format!("{keys} riku deploy key(s) authorized"))
    } else {
        Check::warn(
            NAME,
            "authorized_keys present but no riku deploy keys, add one via riku init",
        )
    }
}

/// Fails unless `dir` itself, not a link to one, is a directory. A missing
/// one passes, so the read reports the missing file.
fn real_directory(dir: &Path) -> io::Result<()> {
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(io::Error::other(format!(
            "{} is not a directory",
            dir.display()
        ))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn read_regular_file(path: &Path) -> io::Result<String> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags((OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK).bits())
        .open(path)
    {
        Ok(file) => file,
        Err(e) => return Err(e),
    };
    match file.metadata() {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err(io::Error::other("not a regular file")),
        Err(e) => return Err(e),
    }
    let mut text = String::new();
    let read = match file.take(MAX_LEN + 1).read_to_string(&mut text) {
        Ok(read) => read,
        Err(e) => return Err(e),
    };
    if read as u64 > MAX_LEN {
        return Err(io::Error::other(format!(
            "larger than {} MiB",
            MAX_LEN / 1024 / 1024
        )));
    }
    Ok(text)
}

#[cfg(test)]
#[path = "ssh_check_tests.rs"]
mod tests;
