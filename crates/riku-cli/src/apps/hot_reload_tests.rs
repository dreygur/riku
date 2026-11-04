use std::fs;
use std::time::{Duration, SystemTime};

use super::*;

#[test]
fn hot_reload_advances_worker_config_mtime_without_altering_content() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());

    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();

    let worker_path = paths.workers_enabled.join("shop@web.1.toml");
    let content = "[worker]\napp = \"shop\"\ncommand = \"python app.py\"\n";
    fs::write(&worker_path, content).unwrap();

    let old_time = match SystemTime::now().checked_sub(Duration::from_secs(3600)) {
        Some(t) => t,
        None => panic!("failed to compute past time"),
    };
    let file = fs::File::options().write(true).open(&worker_path).unwrap();
    file.set_modified(old_time).unwrap();
    drop(file);

    match cmd_hot_reload(&paths, "shop") {
        Ok(()) => {}
        Err(e) => panic!("cmd_hot_reload failed: {e}"),
    }

    let new_mtime = fs::metadata(&worker_path).unwrap().modified().unwrap();
    assert!(
        new_mtime > old_time,
        "expected mtime to advance past {old_time:?}, got {new_mtime:?}"
    );

    let current_content = fs::read_to_string(&worker_path).unwrap();
    assert_eq!(
        current_content, content,
        "expected content to be unchanged, found differences"
    );
}
