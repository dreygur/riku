//! The env file that carries a container's environment to the engine.
//!
//! Security model: values never go on the command line, where any local user
//! could read them from the process list. They are written to a file only the
//! owner can read, which the caller deletes once the engine has read it. The
//! file format is one `KEY=value` per line, so a newline in a value would
//! start a new variable and a `=` or whitespace in a key would change which
//! variable is set. Both are refused here, naming the key and never the value.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Result};
use tempfile::{Builder, TempPath};

/// A name that holds no `=`, whitespace or control character. Shared with
/// build argument names, which have the same `KEY=value` ambiguity.
pub(crate) fn is_valid_name(key: &str) -> bool {
    !key.is_empty()
        && !key
            .chars()
            .any(|c| c == '=' || c.is_whitespace() || c.is_control())
}

fn check_entry(key: &str, value: &str) -> Result<()> {
    if !is_valid_name(key) || key.starts_with('#') {
        return Err(anyhow!(
            "environment variable name {key:?} is not allowed: it must be non-empty, not start with '#', and hold no '=', whitespace or control characters"
        ));
    }
    if value.contains(['\n', '\r', '\0']) {
        return Err(anyhow!(
            "environment variable {key:?} has a value with a newline or NUL character, which an env file cannot hold"
        ));
    }
    Ok(())
}

/// Write `env` to a new mode 0600 file in `dir` and return its path.
///
/// The file is deleted when the returned [`TempPath`] is dropped. Nothing is
/// created when any entry is refused.
pub(crate) fn write_env_file(dir: &Path, env: &BTreeMap<String, String>) -> Result<TempPath> {
    match env
        .iter()
        .try_for_each(|(key, value)| check_entry(key, value))
    {
        Ok(()) => {}
        Err(e) => return Err(e),
    }
    let mut file = match Builder::new().prefix("env-").tempfile_in(dir) {
        Ok(file) => file,
        Err(e) => {
            return Err(anyhow!(
                "cannot create an env file in {}: {e}",
                dir.display()
            ))
        }
    };
    for (key, value) in env {
        if let Err(e) = writeln!(file, "{key}={value}") {
            return Err(anyhow!("cannot write the env file: {e}"));
        }
    }
    if let Err(e) = file.flush() {
        return Err(anyhow!("cannot write the env file: {e}"));
    }
    Ok(file.into_temp_path())
}

#[cfg(test)]
#[path = "env_file_tests.rs"]
mod tests;
