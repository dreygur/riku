//! Test doubles for legacy container worker stacks.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::symlink;

use crate::containers::mock_state::MockServer;
use crate::supervisor::config::create_worker_config;

const LEGACY_COMPOSE_COMMAND: &str = "docker compose -f compose.yaml up";

/// Write a volume state file into the mock engine's state directory.
pub fn write_volume_state(server: &MockServer, volume_name: &str, content: &str) {
    fs::write(server.state.join(format!("vol-{volume_name}")), content).unwrap();
}

/// Engine id of the old stack's container number `index`; never one the
/// mock's own counter hands out.
pub fn legacy_id(index: u32) -> String {
    format!("f{index:063}")
}

fn write_container_state(server: &MockServer, id: &str, name: &str, labels: &str) {
    fs::write(server.state.join(format!("{id}.name")), format!("{name}\n")).unwrap();
    fs::write(server.state.join(format!("{id}.labels")), labels).unwrap();
    fs::write(
        server.state.join(format!("{id}.state")),
        "{\"Status\":\"running\"}\n",
    )
    .unwrap();
}

/// Set up a legacy container stack for myapp on the mock server.
pub fn legacy_stack(server: &MockServer) {
    let app_path = server.paths.app_root.join("myapp");
    let log_file = server.paths.log_root.join("myapp").join("web.1.log");
    let config = create_worker_config(
        "myapp",
        "web",
        LEGACY_COMPOSE_COMMAND,
        1,
        HashMap::from([("PORT".to_string(), "8080".to_string())]),
        &app_path.to_string_lossy(),
        &log_file.to_string_lossy(),
    );
    let toml_str = toml::to_string(&config).unwrap();
    let available = server.paths.workers_available.join("myapp-web-1.toml");
    let enabled = server.paths.workers_enabled.join("myapp-web-1.toml");

    fs::create_dir_all(&server.paths.workers_available).unwrap();
    fs::create_dir_all(&server.paths.workers_enabled).unwrap();
    fs::write(&available, toml_str).unwrap();
    let _ = fs::remove_file(&enabled);
    symlink(&available, &enabled).unwrap();

    let db_id = legacy_id(1);
    let web_id = legacy_id(2);
    let db_labels =
        "\"com.docker.compose.project\":\"myapp\",\"com.docker.compose.service\":\"db\"";
    let web_labels =
        "\"com.docker.compose.project\":\"myapp\",\"com.docker.compose.service\":\"web\"";
    write_container_state(server, &db_id, "myapp-db-1", db_labels);
    write_container_state(server, &web_id, "myapp-web-1", web_labels);

    fs::write(server.state.join("net-myapp_default"), "").unwrap();
    write_volume_state(
        server,
        "myapp_data",
        "{\"com.docker.compose.project\":\"myapp\"}",
    );
}
