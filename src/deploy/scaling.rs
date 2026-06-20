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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_paths(tmp: &TempDir) -> RikuPaths {
        let paths = crate::config::RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        fs::create_dir_all(&paths.workers_enabled).unwrap();
        fs::create_dir_all(&paths.env_root).unwrap();
        paths
    }

    fn make_workers(kinds: &[&str]) -> HashMap<String, String> {
        kinds
            .iter()
            .map(|k| (k.to_string(), format!("{} cmd", k)))
            .collect()
    }

    // --- apply_scaling_deltas ---

    #[test]
    fn test_scaling_up_from_zero_creates_scaling_file() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();

        // No existing SCALING file.  Workers map contains "web", so the
        // `or_insert(1)` default kicks in → baseline = 1, delta = 2 → final = 3.
        let deltas: HashMap<String, i64> = [("web".to_string(), 2i64)].into_iter().collect();
        let workers = make_workers(&["web"]);

        let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers)?;
        assert_eq!(
            counts["web"], 3,
            "baseline 1 (or_insert default) + delta 2 = 3"
        );

        let scaling_path = paths.env_root.join("myapp").join("SCALING");
        assert!(scaling_path.exists(), "SCALING file should be written");
        Ok(())
    }

    #[test]
    fn test_scaling_delta_applied_from_existing_count() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir).unwrap();
        fs::write(env_dir.join("SCALING"), "web=3\n")?;

        let deltas: HashMap<String, i64> = [("web".to_string(), 1i64)].into_iter().collect();
        let workers = make_workers(&["web"]);

        let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers)?;
        assert_eq!(counts["web"], 4, "3 existing + delta 1 = 4");
        Ok(())
    }

    #[test]
    fn test_scaling_down_removes_symlinks() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir).unwrap();
        fs::write(env_dir.join("SCALING"), "web=3\n")?;

        // Create dummy symlink files that should be removed on scale-down
        let file2 = paths.workers_enabled.join("myapp-web-2.toml");
        let file3 = paths.workers_enabled.join("myapp-web-3.toml");
        fs::write(&file2, "[worker]\n")?;
        fs::write(&file3, "[worker]\n")?;

        let deltas: HashMap<String, i64> = [("web".to_string(), -2i64)].into_iter().collect();
        let workers = make_workers(&["web"]);

        let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers)?;
        assert_eq!(counts["web"], 1, "3 - 2 = 1");
        assert!(!file2.exists(), "myapp-web-2.toml should have been removed");
        assert!(!file3.exists(), "myapp-web-3.toml should have been removed");
        Ok(())
    }

    #[test]
    fn test_scaling_to_zero_saturates_at_zero() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir).unwrap();
        fs::write(env_dir.join("SCALING"), "web=1\n")?;

        // delta of -999 should saturate at 0, not underflow
        let deltas: HashMap<String, i64> = [("web".to_string(), -999i64)].into_iter().collect();
        let workers = make_workers(&["web"]);

        let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers)?;
        assert_eq!(
            counts["web"], 0,
            "Should saturate at 0 (no negative workers)"
        );
        Ok(())
    }

    #[test]
    fn test_scaling_default_when_no_scaling_file() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();

        // No SCALING file, no deltas → workers default to 1
        let deltas: HashMap<String, i64> = HashMap::new();
        let workers = make_workers(&["web", "worker"]);

        let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers)?;
        assert_eq!(counts.get("web").copied().unwrap_or(0), 1);
        assert_eq!(counts.get("worker").copied().unwrap_or(0), 1);
        Ok(())
    }

    #[test]
    fn test_scaling_file_written_with_sorted_keys() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();

        let deltas: HashMap<String, i64> =
            [("worker".to_string(), 2i64), ("web".to_string(), 3i64)]
                .into_iter()
                .collect();
        let workers = make_workers(&["web", "worker"]);

        apply_scaling_deltas("myapp", &paths, &deltas, &workers)?;

        let content = fs::read_to_string(paths.env_root.join("myapp").join("SCALING"))?;
        let lines: Vec<&str> = content.lines().collect();
        // Sorted alphabetically: web < worker
        assert!(
            lines.iter().position(|l| l.starts_with("web=")).unwrap()
                < lines.iter().position(|l| l.starts_with("worker=")).unwrap(),
            "SCALING file keys should be sorted"
        );
        Ok(())
    }

    #[test]
    fn test_scaling_up_does_not_remove_symlinks() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir).unwrap();
        fs::write(env_dir.join("SCALING"), "web=2\n")?;

        let existing = paths.workers_enabled.join("myapp-web-1.toml");
        fs::write(&existing, "[worker]\n")?;

        let deltas: HashMap<String, i64> = [("web".to_string(), 1i64)].into_iter().collect();
        let workers = make_workers(&["web"]);
        apply_scaling_deltas("myapp", &paths, &deltas, &workers)?;

        assert!(
            existing.exists(),
            "Scaling up should never remove existing configs"
        );
        Ok(())
    }
}
