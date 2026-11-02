use crate::containers::legacy::is_legacy_worker;
use crate::supervisor::config::create_worker_config;
use std::collections::HashMap;

fn make_config(app: &str, command: &str) -> crate::supervisor::config::WorkerConfig {
    create_worker_config(
        app,
        "web",
        command,
        1,
        HashMap::new(),
        "/srv/app",
        "/var/log/app.log",
    )
}

#[test]
fn recognises_only_plugin_commands() {
    let myapp_docker_compose = make_config("myapp", "docker compose -f compose.yaml up");
    assert!(
        is_legacy_worker(&myapp_docker_compose),
        "docker compose command must be recognized"
    );

    let myapp_podman_compose = make_config("myapp", "podman compose -f docker-compose.yml up");
    assert!(
        is_legacy_worker(&myapp_podman_compose),
        "podman compose command must be recognized"
    );

    let myapp_podman_dash_compose = make_config("myapp", "podman-compose -f compose.yaml up");
    assert!(
        is_legacy_worker(&myapp_podman_dash_compose),
        "podman-compose command must be recognized"
    );

    let myapp_docker_run = make_config("myapp", "docker run --rm -p $PORT:$PORT riku-myapp");
    assert!(
        is_legacy_worker(&myapp_docker_run),
        "docker run command must be recognized"
    );

    let myapp_podman_run = make_config("myapp", "podman run --rm -p $PORT:$PORT riku-myapp");
    assert!(
        is_legacy_worker(&myapp_podman_run),
        "podman run command must be recognized"
    );

    let python = make_config("myapp", "python app.py");
    assert!(
        !is_legacy_worker(&python),
        "python command must not be recognized as legacy"
    );

    let compose_detached = make_config("myapp", "docker compose -f x up -d");
    assert!(
        !is_legacy_worker(&compose_detached),
        "detached compose command must not be recognized as legacy"
    );

    let other_app_run = make_config("myapp", "docker run --rm -p $PORT:$PORT riku-other");
    assert!(
        !is_legacy_worker(&other_app_run),
        "command for another app must not be recognized as legacy for myapp"
    );
}
