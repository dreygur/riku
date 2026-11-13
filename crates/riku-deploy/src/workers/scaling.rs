//! Reading worker process scaling counts from the SCALING configuration file.

use anyhow::{anyhow, Result};
use std::fs;

use crate::config::RikuPaths;

/// Read the scaling count for a given process kind from the SCALING file.
///
/// Returns 1 if the file doesn't exist or the kind isn't listed. A line for
/// `kind` whose value is not a whole number is an error naming the line, so
/// a typo does not silently run one copy.
pub fn read_scaling_count(paths: &RikuPaths, app: &str, kind: &str) -> Result<u32> {
    let scaling_path = paths.env_root.join(app).join("SCALING");
    if !scaling_path.exists() {
        return Ok(1);
    }
    let content = match fs::read_to_string(&scaling_path) {
        Ok(c) => c,
        Err(e) => return Err(e.into()),
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            if key.trim() == kind {
                return match val.trim().parse::<u32>() {
                    Ok(n) => Ok(n),
                    Err(_) => Err(anyhow!(
                        "{}: '{line}' is not a copy count",
                        scaling_path.display()
                    )),
                };
            }
        }
    }
    Ok(1)
}
