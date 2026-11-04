use super::*;
use std::fs;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    let paths = crate::config::RikuPaths::for_tests(tmp.path());
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

#[test]
fn test_scaling_up_from_zero_creates_scaling_file() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();

    // No existing SCALING file.  Workers map contains "web", so the
    // `or_insert(1)` default kicks in → baseline = 1, delta = 2 → final = 3.
    let deltas: HashMap<String, i64> = [("web".to_string(), 2i64)].into_iter().collect();
    let workers = make_workers(&["web"]);

    let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers).unwrap();
    assert_eq!(
        counts["web"], 3,
        "baseline 1 (or_insert default) + delta 2 = 3"
    );

    let scaling_path = paths.env_root.join("myapp").join("SCALING");
    assert!(scaling_path.exists(), "SCALING file should be written");
}

#[test]
fn test_scaling_delta_applied_from_existing_count() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("SCALING"), "web=3\n").unwrap();

    let deltas: HashMap<String, i64> = [("web".to_string(), 1i64)].into_iter().collect();
    let workers = make_workers(&["web"]);

    let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers).unwrap();
    assert_eq!(counts["web"], 4, "3 existing + delta 1 = 4");
}

#[test]
fn test_scaling_down_removes_symlinks() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("SCALING"), "web=3\n").unwrap();

    let file2 = paths.workers_enabled.join("myapp@web.2.toml");
    let file3 = paths.workers_enabled.join("myapp@web.3.toml");
    fs::write(&file2, "[worker]\n").unwrap();
    fs::write(&file3, "[worker]\n").unwrap();

    let deltas: HashMap<String, i64> = [("web".to_string(), -2i64)].into_iter().collect();
    let workers = make_workers(&["web"]);

    let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers).unwrap();
    assert_eq!(counts["web"], 1, "3 - 2 = 1");
    assert!(!file2.exists(), "myapp@web.2.toml should have been removed");
    assert!(!file3.exists(), "myapp@web.3.toml should have been removed");
}

#[test]
fn test_scaling_to_zero_saturates_at_zero() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("SCALING"), "web=1\n").unwrap();

    let deltas: HashMap<String, i64> = [("web".to_string(), -999i64)].into_iter().collect();
    let workers = make_workers(&["web"]);

    let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers).unwrap();
    assert_eq!(
        counts["web"], 0,
        "Should saturate at 0 (no negative workers)"
    );
}

#[test]
fn test_scaling_default_when_no_scaling_file() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();

    let deltas: HashMap<String, i64> = HashMap::new();
    let workers = make_workers(&["web", "worker"]);

    let counts = apply_scaling_deltas("myapp", &paths, &deltas, &workers).unwrap();
    assert_eq!(counts.get("web").copied().unwrap_or(0), 1);
    assert_eq!(counts.get("worker").copied().unwrap_or(0), 1);
}

#[test]
fn test_scaling_file_written_with_sorted_keys() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();

    let deltas: HashMap<String, i64> = [("worker".to_string(), 2i64), ("web".to_string(), 3i64)]
        .into_iter()
        .collect();
    let workers = make_workers(&["web", "worker"]);

    apply_scaling_deltas("myapp", &paths, &deltas, &workers).unwrap();

    let content = fs::read_to_string(paths.env_root.join("myapp").join("SCALING")).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert!(
        lines.iter().position(|l| l.starts_with("web=")).unwrap()
            < lines.iter().position(|l| l.starts_with("worker=")).unwrap(),
        "SCALING file keys should be sorted"
    );
}

#[test]
fn test_scaling_up_does_not_remove_symlinks() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("SCALING"), "web=2\n").unwrap();

    let existing = paths.workers_enabled.join("myapp@web.1.toml");
    fs::write(&existing, "[worker]\n").unwrap();

    let deltas: HashMap<String, i64> = [("web".to_string(), 1i64)].into_iter().collect();
    let workers = make_workers(&["web"]);
    apply_scaling_deltas("myapp", &paths, &deltas, &workers).unwrap();

    assert!(
        existing.exists(),
        "Scaling up should never remove existing configs"
    );
}
