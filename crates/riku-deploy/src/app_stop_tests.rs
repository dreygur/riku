use std::collections::HashMap;
use std::fs;

use tempfile::TempDir;

use super::*;
use crate::supervisor::config::create_worker_config;

/// Writes a wsgi copy the way a deploy does: an available config, an enabled
/// link to it and the copy's socket.
fn deployed_wsgi_copy(paths: &RikuPaths, copy: u32) -> [PathBuf; 3] {
    let socket = crate::router::copy_socket(paths, "myapp", CopyIndex::new(copy).unwrap());
    let env = HashMap::from([(
        "UWSGI_SOCKET".to_string(),
        socket.to_string_lossy().to_string(),
    )]);
    let config = create_worker_config("myapp", "wsgi", "run", copy, env, "/srv/app", "/srv/log");
    let name = format!("myapp@wsgi.{copy}.toml");
    let available = paths.workers_available.join(&name);
    let enabled = paths.workers_enabled.join(&name);
    fs::write(&available, toml::to_string(&config).unwrap()).unwrap();
    std::os::unix::fs::symlink(&available, &enabled).unwrap();
    fs::write(&socket, "").unwrap();
    [available, enabled, socket]
}

#[test]
fn stop_removes_every_copys_config_and_socket() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    for dir in [
        &paths.workers_available,
        &paths.workers_enabled,
        &paths.socket_root,
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    let left: Vec<PathBuf> = (1..=2)
        .flat_map(|copy| deployed_wsgi_copy(&paths, copy))
        .collect();

    let stopped = stop_procfile_app("myapp", &paths).unwrap();

    assert_eq!(stopped, 2);
    let remaining: Vec<&PathBuf> = left.iter().filter(|path| path.exists()).collect();
    assert!(remaining.is_empty(), "left behind: {remaining:?}");
}
