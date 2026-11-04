//! Test double for the Procfile workers of an app switching to containers.

use std::collections::HashMap;
use std::fs;
use std::net::TcpListener;
use std::os::unix::fs::symlink;

use crate::containers::mock_state::MockServer;
use crate::supervisor::config::create_worker_config;

/// Set up an enabled Procfile web worker for myapp on the mock server.
#[must_use]
pub fn procfile_workers(server: &MockServer) -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let app_path = server.paths.app_root.join("myapp");
    let log_file = server.paths.log_root.join("myapp").join("web.1.log");
    let config = create_worker_config(
        "myapp",
        "web",
        "python app.py",
        1,
        HashMap::from([("PORT".to_string(), port.to_string())]),
        &app_path.to_string_lossy(),
        &log_file.to_string_lossy(),
    );
    let toml_str = toml::to_string(&config).unwrap();
    let available = server.paths.workers_available.join("myapp@web.1.toml");
    let enabled = server.paths.workers_enabled.join("myapp@web.1.toml");

    fs::create_dir_all(&server.paths.workers_available).unwrap();
    fs::create_dir_all(&server.paths.workers_enabled).unwrap();
    fs::write(&available, toml_str).unwrap();
    let _ = fs::remove_file(&enabled);
    symlink(&available, &enabled).unwrap();
    listener
}
