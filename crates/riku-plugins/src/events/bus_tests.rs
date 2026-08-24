use super::*;
use std::os::unix::fs::PermissionsExt;

fn write_exec(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn manifest_toml(name: &str, event: &str) -> String {
    format!(
            "name=\"{name}\"\nversion=\"1\"\ntype=\"notifier\"\napi={RIKU_PLUGIN_API}\nentry=\"bin/on-event\"\n[events]\nsubscribe=[\"{event}\"]\n"
        )
}

fn manifest_toml_with_priority(name: &str, event: &str, priority: i32) -> String {
    format!(
            "name=\"{name}\"\nversion=\"1\"\ntype=\"notifier\"\napi={RIKU_PLUGIN_API}\nentry=\"bin/on-event\"\n[events]\nsubscribe=[\"{event}\"]\npriority={priority}\n"
        )
}

fn make_bus_paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
    (tmp, paths)
}

#[test]
fn observe_subscriber_receives_event_on_stdin() {
    let (tmp, paths) = make_bus_paths();
    let bundle = paths.plugin_root.join("recorder");
    std::fs::create_dir_all(bundle.join("bin")).unwrap();
    let received = tmp.path().join("received.json");
    write_exec(
        &bundle.join("bin/on-event"),
        &format!("#!/bin/sh\ncat > '{}'\n", received.display()),
    );
    std::fs::write(
        bundle.join("riku-plugin.toml"),
        manifest_toml("recorder", "deploy.finished"),
    )
    .unwrap();

    EventBus::new(&paths).publish(
        EventName::DeployFinished,
        "myapp",
        serde_json::json!({ "k": "v" }),
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&received).unwrap()).unwrap();
    assert_eq!(parsed["event"], "deploy.finished");
    assert_eq!(parsed["app"], "myapp");
    assert_eq!(parsed["data"]["k"], "v");
    assert_eq!(parsed["api"], RIKU_PLUGIN_API);
}

#[test]
fn subscribers_run_in_priority_order() {
    let (tmp, paths) = make_bus_paths();
    let marker = tmp.path().join("order.log");

    let make_bundle = |name: &str, priority: i32| {
        let bundle = paths.plugin_root.join(name);
        std::fs::create_dir_all(bundle.join("bin")).unwrap();
        write_exec(
            &bundle.join("bin/on-event"),
            &format!("#!/bin/sh\necho {name} >> '{}'\n", marker.display()),
        );
        std::fs::write(
            bundle.join("riku-plugin.toml"),
            manifest_toml_with_priority(name, "deploy.finished", priority),
        )
        .unwrap();
    };

    // Installed alphabetically reversed and out of priority order, so a
    // filesystem-order or name-order pass would get this wrong too.
    make_bundle("second", 5);
    make_bundle("first", 1);

    EventBus::new(&paths).publish(EventName::DeployFinished, "app", serde_json::json!({}));

    let order = std::fs::read_to_string(&marker).unwrap();
    assert_eq!(order, "first\nsecond\n");
}

#[test]
fn subscriber_to_other_event_is_not_invoked() {
    let (tmp, paths) = make_bus_paths();
    let bundle = paths.plugin_root.join("other");
    std::fs::create_dir_all(bundle.join("bin")).unwrap();
    let marker = tmp.path().join("ran");
    write_exec(
        &bundle.join("bin/on-event"),
        &format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    );
    std::fs::write(
        bundle.join("riku-plugin.toml"),
        manifest_toml("other", "build.started"),
    )
    .unwrap();

    EventBus::new(&paths).publish(EventName::DeployFinished, "app", serde_json::json!({}));
    assert!(
        !marker.exists(),
        "a subscriber to a different event must not run"
    );
}

#[test]
fn emit_is_safe_with_no_plugins_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
    // plugin_root does not exist: must not panic.
    EventBus::new(&paths).publish(EventName::DeployFinished, "app", serde_json::json!({}));
}

#[test]
fn invalid_bundle_is_skipped_without_panicking() {
    let (_tmp, paths) = make_bus_paths();
    let bundle = paths.plugin_root.join("broken");
    std::fs::create_dir_all(&bundle).unwrap();
    // api 999 is unsupported → manifest invalid → bundle skipped.
    std::fs::write(
        bundle.join("riku-plugin.toml"),
        "name=\"broken\"\nversion=\"1\"\ntype=\"notifier\"\napi=999\nentry=\"x\"\n",
    )
    .unwrap();
    EventBus::new(&paths).publish(EventName::DeployFinished, "app", serde_json::json!({}));
}

/// End-to-end: the real `plugins/riku-notify` bundle shipped in the repo,
/// installed into a temp plugin root exactly as `riku install-plugins`
/// would lay it out, actually reaches a webhook when `app.restarted`
/// fires: through the *real* sandbox (`capabilities.network = true`
/// must actually let curl's DNS/TCP through under landlock), not just a
/// direct invocation of the script.
#[test]
fn bundled_riku_notify_plugin_delivers_webhook_on_app_restarted() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let (_tmp, paths) = make_bus_paths();

    // Copy the repo's real plugins/riku-notify/ bundle into the temp
    // plugin root: the actual shipped manifest + script, not a fixture.
    let repo_bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/riku-notify");
    let dest = paths.plugin_root.join("riku-notify");
    std::fs::create_dir_all(dest.join("bin")).unwrap();
    std::fs::copy(
        repo_bundle.join("riku-plugin.toml"),
        dest.join("riku-plugin.toml"),
    )
    .unwrap();
    std::fs::copy(repo_bundle.join("bin/on-event"), dest.join("bin/on-event")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dest.join("bin/on-event"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }

    // Minimal one-shot HTTP server: accept a single connection, read the
    // request, hand back 200. No new dev-dependency for this.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
        String::from_utf8_lossy(&buf[..n]).to_string()
    });

    std::env::set_var(
        "RIKU_NOTIFY_WEBHOOK_URL",
        format!("http://127.0.0.1:{port}/incident"),
    );

    EventBus::new(&paths).publish(
        EventName::AppRestarted,
        "demo-app",
        serde_json::json!({ "instance": "demo-app.web.0", "exit_code": 137, "restart_count": 2 }),
    );

    std::env::remove_var("RIKU_NOTIFY_WEBHOOK_URL");

    let request = received.join().unwrap();
    assert!(request.contains("POST /incident"), "got: {request}");
    assert!(
        request.contains("\"instance\":\"demo-app.web.0\""),
        "got: {request}"
    );
    assert!(request.contains("\"exit_code\":137"), "got: {request}");
    assert!(request.contains("\"restart_count\":2"), "got: {request}");
}
