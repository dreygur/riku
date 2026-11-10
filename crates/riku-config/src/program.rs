//! Locating host executables.
//!
//! Security model: the names passed in are compile-time constants such as
//! `nginx` or `systemctl`. No app value or user input reaches the lookup, and
//! the search directories are `$PATH` plus a fixed list of system directories.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Searched after `$PATH`. An ssh or git-hook session on Debian has no
/// `/usr/sbin` in `PATH`, which is where `nginx` lives.
const FALLBACK_DIRS: &[&str] = &[
    "/usr/local/sbin",
    "/usr/sbin",
    "/sbin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/snap/bin",
];

fn is_executable(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

fn find_in(name: &str, dirs: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    dirs.into_iter()
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

/// The first executable regular file called `name` in `$PATH`, then in the
/// standard system directories; `None` when the program is not installed.
pub fn installed_program(name: &str) -> Option<PathBuf> {
    let path_dirs = match std::env::var_os("PATH") {
        Some(value) => std::env::split_paths(&value).collect::<Vec<_>>(),
        None => Vec::new(),
    };
    let dirs = path_dirs
        .into_iter()
        .chain(FALLBACK_DIRS.iter().map(PathBuf::from));
    find_in(name, dirs)
}

/// Find `name` in `$PATH`, then in the standard system directories.
///
/// Returns the first executable regular file found. When there is none, the
/// bare `name` comes back unchanged, so spawning it fails with the OS's own
/// "not found" error.
pub fn find_program(name: &str) -> PathBuf {
    match installed_program(name) {
        Some(found) => found,
        None => PathBuf::from(name),
    }
}

#[cfg(test)]
#[path = "program_tests.rs"]
mod tests;
