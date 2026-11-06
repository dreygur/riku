use super::*;
use std::fs;
use tempfile::TempDir;

fn paths_for(root: &std::path::Path) -> RikuPaths {
    let paths = RikuPaths::for_tests(root);
    fs::create_dir_all(&paths.workers_available).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    paths
}

fn seed_worker(paths: &RikuPaths, app: &str, kind: &str, ordinal: u32) {
    let filename = WorkerName::new(app, kind, ordinal).file_name();
    let available = paths.workers_available.join(&filename);
    fs::write(&available, "placeholder").unwrap();
    std::os::unix::fs::symlink(&available, paths.workers_enabled.join(&filename)).unwrap();
}

#[test]
fn restart_swaps_the_symlink_without_touching_content() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    seed_worker(&paths, "myapp", "web", 2);

    restart_worker("myapp", &paths, "web", 2).unwrap();

    let enabled = paths.workers_enabled.join("myapp@web.2.toml");
    assert!(enabled.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&enabled).unwrap(), "placeholder");
}

#[test]
fn restart_errors_when_config_was_scaled_away() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());

    let err = restart_worker("myapp", &paths, "web", 5).unwrap_err();
    assert!(err.to_string().contains("scaled away"));
}

fn seed_scaling(paths: &RikuPaths, app: &str, kind: &str, count: u32) {
    let dir = paths.env_root.join(app);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SCALING"), format!("{kind}={count}\n")).unwrap();
}

#[test]
fn delete_removes_the_highest_ordinal_and_updates_scaling() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    seed_worker(&paths, "myapp", "web", 1);
    seed_worker(&paths, "myapp", "web", 2);
    seed_scaling(&paths, "myapp", "web", 2);

    delete_worker("myapp", &paths, "web", 2).unwrap();

    assert!(paths.workers_enabled.join("myapp@web.1.toml").exists());
    // apply_scaling_deltas only unlinks the enabled symlink (stopping the
    // process) -- the workers_available file is left behind, same as
    // any other scale-down; that's an existing, unrelated behavior.
    assert!(!paths.workers_enabled.join("myapp@web.2.toml").exists());
    assert_eq!(read_scaling_count(&paths, "myapp", "web").unwrap(), 1);
}

#[test]
fn delete_refuses_a_non_highest_ordinal() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    seed_worker(&paths, "myapp", "web", 1);
    seed_worker(&paths, "myapp", "web", 2);
    seed_scaling(&paths, "myapp", "web", 2);

    let err = delete_worker("myapp", &paths, "web", 1).unwrap_err();
    assert!(err.to_string().contains("isn't the highest instance"));
    // Nothing was touched.
    assert!(paths.workers_enabled.join("myapp@web.1.toml").exists());
    assert!(paths.workers_enabled.join("myapp@web.2.toml").exists());
}

#[test]
fn rejects_invalid_kind() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    assert!(restart_worker("myapp", &paths, "../etc", 1).is_err());
    assert!(delete_worker("myapp", &paths, "web/../x", 1).is_err());
}
