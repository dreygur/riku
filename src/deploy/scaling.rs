//! Worker scaling: read/apply scaling deltas, prune symlinks.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;

use crate::config::RikuPaths;
use crate::util::echo;

/// Apply scaling deltas to the SCALING file and return the new worker counts.
/// Also removes symlinks for workers that have been scaled down.
pub(crate) fn apply_scaling_deltas(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    workers: &HashMap<String, String>,
) -> Result<HashMap<String, u32>> {
    let scaling_path = paths.env_root.join(app).join("SCALING");
    let mut worker_counts: HashMap<String, u32> = HashMap::new();

    // Read current scaling values
    if scaling_path.exists() {
        let content = fs::read_to_string(&scaling_path)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim();
                let val = line[pos + 1..].trim();
                if let Ok(count) = val.parse::<u32>() {
                    worker_counts.insert(key.to_string(), count);
                }
            }
        }
    }

    // Default to 1 for any worker types not in SCALING
    for kind in workers.keys() {
        worker_counts.entry(kind.clone()).or_insert(1);
    }

    // Apply deltas
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
        echo(
            &format!(
                "-----> Scaling '{}': {} -> {} (delta: {})",
                kind, current, new_count, delta
            ),
            "green",
        );
    }

    // Write new scaling file
    let mut scaling_content = String::new();
    let mut counts: Vec<_> = new_counts.iter().collect();
    counts.sort();
    for (kind, count) in counts {
        scaling_content.push_str(&format!("{}={}\n", kind, count));
    }
    fs::create_dir_all(paths.env_root.join(app))?;
    fs::write(&scaling_path, &scaling_content)?;

    // Remove symlinks for scaled-down workers
    for (kind, new_count) in &new_counts {
        let old_count = *worker_counts.get(kind).unwrap_or(&1);
        if new_count < &old_count {
            for ordinal in (*new_count + 1)..=old_count {
                let config_filename = format!("{}-{}-{}.toml", app, kind, ordinal);
                let enabled_path = paths.workers_enabled.join(&config_filename);
                if enabled_path.exists() {
                    fs::remove_file(&enabled_path)?;
                    echo(
                        &format!(
                            "-----> Removed worker config: {} (scaled down)",
                            config_filename
                        ),
                        "yellow",
                    );
                }
            }
        }
    }

    Ok(new_counts)
}
