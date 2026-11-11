//! `riku doctor` check that a riku deploy key is authorized for `git push`.
//!
//! Security: under `sudo riku doctor` this reads the deploy user's
//! `authorized_keys` as root. The file is opened with `O_NOFOLLOW` and
//! `O_NONBLOCK` and must be a regular file, so a symlink or FIFO planted at
//! that name is refused instead of followed or waited on. Only a count of
//! riku lines leaves this module.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use nix::fcntl::OFlag;

use crate::util::authorized_keys::{authorized_keys_path, RIKU_KEY_MARKER};

use super::Check;

const NAME: &str = "ssh access";

/// Count the riku deploy keys in the `authorized_keys` of the user whose
/// home is `home`.
pub fn ssh_access(home: &Path) -> Check {
    let path = authorized_keys_path(home);
    let text = match read_regular_file(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Check::warn(
                NAME,
                "no authorized_keys: git push deploys need a key (riku init)",
            )
        }
        Err(e) => return Check::warn(NAME, format!("could not read {}: {e}", path.display())),
    };
    let keys = text
        .lines()
        .filter(|line| line.contains(RIKU_KEY_MARKER))
        .count();
    if keys > 0 {
        Check::ok(NAME, format!("{keys} riku deploy key(s) authorized"))
    } else {
        Check::warn(
            NAME,
            "authorized_keys present but no riku deploy keys, add one via riku init",
        )
    }
}

fn read_regular_file(path: &Path) -> io::Result<String> {
    let mut file = match OpenOptions::new()
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
    match file.read_to_string(&mut text) {
        Ok(_) => Ok(text),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "ssh_check_tests.rs"]
mod tests;
