//! Finding the container engine executable.
//!
//! Security model: the `engine.program` setting is operator input that ends
//! up as the program riku executes. It is accepted only as an absolute path
//! to an executable file or as a bare name resolved against the supplied
//! search path. Relative paths and anything containing whitespace are refused,
//! and relative search path entries are skipped, so the working directory
//! never decides which binary runs.

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

/// Engines tried, in this order, when no engine is configured.
pub const KNOWN_ENGINES: [&str; 4] = ["docker", "podman", "nerdctl", "finch"];

/// A resolved engine executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineBinary {
    /// Absolute path of the executable.
    pub program: PathBuf,
    /// File name of the executable, for messages only. Driver behavior must
    /// not branch on it.
    pub binary_name: String,
}

/// Resolve the engine to drive.
///
/// `setting` is the settings.toml `engine.program` value. With no setting, the first of
/// [`KNOWN_ENGINES`] found in `search_path` wins, so list order beats `PATH`
/// order. The caller passes `PATH`; this function reads no environment.
pub fn detect(setting: Option<&str>, search_path: &OsStr) -> Result<EngineBinary> {
    match setting {
        Some(value) => from_setting(value, search_path),
        None => from_known(search_path),
    }
}

fn from_setting(value: &str, search_path: &OsStr) -> Result<EngineBinary> {
    let path = Path::new(value);
    if path.is_absolute() {
        if !is_executable(path) {
            return Err(anyhow!(
                "engine.program is {value}, which is not an executable file"
            ));
        }
        return Ok(binary_at(path.to_path_buf()));
    }
    if value.contains('/') || value.contains(char::is_whitespace) {
        return Err(anyhow!(
            "engine.program must be an engine name or an absolute path, got {value:?}"
        ));
    }
    match find_in(value, search_path) {
        Some(program) => Ok(binary_at(program)),
        None => Err(anyhow!(
            "engine.program is {value}, but no executable with that name is on the search path"
        )),
    }
}

fn from_known(search_path: &OsStr) -> Result<EngineBinary> {
    for name in KNOWN_ENGINES {
        if let Some(program) = find_in(name, search_path) {
            return Ok(binary_at(program));
        }
    }
    Err(anyhow!(
        "no container engine found (looked for {} on the search path). \
         Install one or set engine.program in settings.toml",
        KNOWN_ENGINES.join(", ")
    ))
}

fn find_in(name: &str, search_path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(search_path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    match path.metadata() {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

fn binary_at(program: PathBuf) -> EngineBinary {
    let binary_name = program
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    EngineBinary {
        program,
        binary_name,
    }
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
