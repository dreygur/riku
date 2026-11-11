//! Worker scaling: read/apply scaling deltas, prune symlinks.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;

use crate::config::RikuPaths;
use crate::util::display;
use crate::util::file_names::worker::WorkerName;

/// The counts in an app's SCALING file, `kind=count` per line.
fn read_counts(scaling_path: &std::path::Path) -> Result<HashMap<String, u32>> {
    let mut counts = HashMap::new();
    if !scaling_path.exists() {
        return Ok(counts);
    }
    let content = match fs::read_to_string(scaling_path) {
        Ok(content) => content,
        Err(e) => return Err(anyhow!("cannot read {}: {e}", scaling_path.display())),
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            if let Ok(count) = val.trim().parse::<u32>() {
                counts.insert(key.trim().to_string(), count);
            }
        }
    }
    Ok(counts)
}

/// Unlink the enabled configs of copies `from..=to` of `kind`.
fn remove_scaled_down(app: &str, kind: &str, from: u32, to: u32, paths: &RikuPaths) -> Result<()> {
    for ordinal in from..=to {
        let config_filename = WorkerName::new(app, kind, ordinal).file_name();
        let enabled_path = paths.workers_enabled.join(&config_filename);
        if !enabled_path.exists() {
            continue;
        }
        if let Err(e) = fs::remove_file(&enabled_path) {
            return Err(anyhow!("cannot remove {}: {e}", enabled_path.display()));
        }
        display::detail(&format!("Removed {config_filename} (scaled down)"));
    }
    Ok(())
}

/// Apply scaling deltas to the SCALING file and return the new worker counts.
/// Also removes symlinks for workers that have been scaled down.
#[allow(clippy::question_mark)]
pub(crate) fn apply_scaling_deltas(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    workers: &HashMap<String, String>,
) -> Result<HashMap<String, u32>> {
    let scaling_path = paths.env_root.join(app).join("SCALING");
    let mut worker_counts = match read_counts(&scaling_path) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };

    // Default to 1 for any worker types not in SCALING
    for kind in workers.keys() {
        worker_counts.entry(kind.clone()).or_insert(1);
    }

    let mut new_counts: HashMap<String, u32> = worker_counts.clone();
    for (kind, delta) in deltas {
        // Use 0 as the baseline for kinds not yet in the SCALING file so that
        // "web=2" on a fresh app gives exactly 2 workers (not 1+2=3).
        let current = *worker_counts.get(kind).unwrap_or(&0);
        let new_count = if *delta < 0 {
            current.saturating_sub((-delta) as u32)
        } else {
            current + (*delta as u32)
        };
        new_counts.insert(kind.clone(), new_count);
        display::detail(&format!("Scaling {kind}: {current} to {new_count}"));
    }

    let mut counts: Vec<_> = new_counts.iter().collect();
    counts.sort();
    let scaling_content: String = counts
        .into_iter()
        .map(|(kind, count)| format!("{kind}={count}\n"))
        .collect();
    if let Err(e) = crate::util::create_private_dir(&paths.env_root.join(app)) {
        return Err(anyhow!("cannot create the env dir of '{app}': {e}"));
    }
    if let Err(e) = fs::write(&scaling_path, &scaling_content) {
        return Err(anyhow!("cannot write {}: {e}", scaling_path.display()));
    }

    for (kind, new_count) in &new_counts {
        let old_count = *worker_counts.get(kind).unwrap_or(&1);
        if *new_count < old_count {
            if let Err(e) = remove_scaled_down(app, kind, new_count + 1, old_count, paths) {
                return Err(e);
            }
        }
    }

    Ok(new_counts)
}

#[cfg(test)]
#[path = "scaling_tests.rs"]
mod tests;
