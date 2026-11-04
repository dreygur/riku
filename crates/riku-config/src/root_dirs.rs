//! Validating the two path inputs riku takes from outside, `HOME` and
//! `RIKU_ROOT`: each must be absolute with no `..`.

use std::path::{Component, PathBuf};

use anyhow::{anyhow, Result};

fn checked_dir(var: &str, value: String) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    let climbs = path.components().any(|c| c == Component::ParentDir);
    if !path.is_absolute() || climbs {
        return Err(anyhow!(
            "{var} must be an absolute path without `..`, got {}",
            path.display()
        ));
    }
    Ok(path)
}

/// Resolve `(riku_root, home)` from the raw `HOME` and `RIKU_ROOT` values.
///
/// Kept apart from `from_env` so the validation can be tested without
/// mutating the process environment.
#[allow(clippy::question_mark)]
pub(crate) fn root_dirs(
    home: Option<String>,
    riku_root: Option<String>,
) -> Result<(PathBuf, PathBuf)> {
    let home = match home {
        Some(value) => match checked_dir("HOME", value) {
            Ok(path) => path,
            Err(e) => return Err(e),
        },
        None => return Err(anyhow!("HOME environment variable must be set")),
    };
    let riku_root = match riku_root {
        Some(value) => match checked_dir("RIKU_ROOT", value) {
            Ok(path) => path,
            Err(e) => return Err(e),
        },
        None => home.join(".riku"),
    };
    Ok((riku_root, home))
}
