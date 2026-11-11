use super::*;
use std::os::unix::fs::PermissionsExt;

fn write_exec(path: &std::path::Path, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn plugin(name: &str) -> Option<String> {
    Some(name.to_string())
}

#[test]
fn rejects_non_namespaced_event_names() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());

    let err = emit_as(&paths, plugin("whatever"), "app.restarted", "{}", None)
        .unwrap_err()
        .to_string();

    assert!(err.contains("plugin.custom."), "got: {err}");
}

#[test]
fn rejects_plugin_without_emit_declared() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());

    let bundle = paths.plugin_root.join("quiet");
    std::fs::create_dir_all(bundle.join("bin")).unwrap();
    write_exec(&bundle.join("bin/on-event"), "#!/bin/sh\nexit 0\n");
    std::fs::write(
        bundle.join("riku-plugin.toml"),
        format!(
            "name=\"quiet\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-event\"\n",
            riku_plugins::RIKU_PLUGIN_API
        ),
    )
    .unwrap();

    let err = emit_as(&paths, plugin("quiet"), "plugin.custom.thing", "{}", None)
        .unwrap_err()
        .to_string();

    assert!(err.contains("emit = true"), "got: {err}");
}

#[test]
fn emitter_reaches_a_real_subscriber_with_source_plugin_set() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());

    // Plugin A: allowed to emit.
    let emitter = paths.plugin_root.join("emitter");
    std::fs::create_dir_all(emitter.join("bin")).unwrap();
    write_exec(&emitter.join("bin/on-event"), "#!/bin/sh\nexit 0\n");
    std::fs::write(
        emitter.join("riku-plugin.toml"),
        format!(
            "name=\"emitter\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-event\"\n[events]\nemit=true\n",
            riku_plugins::RIKU_PLUGIN_API
        ),
    )
    .unwrap();

    // Plugin B: subscribed to the custom event, records what it received.
    let received = tmp.path().join("received.json");
    let subscriber = paths.plugin_root.join("subscriber");
    std::fs::create_dir_all(subscriber.join("bin")).unwrap();
    write_exec(
        &subscriber.join("bin/on-event"),
        &format!("#!/bin/sh\ncat > '{}'\n", received.display()),
    );
    std::fs::write(
        subscriber.join("riku-plugin.toml"),
        format!(
            "name=\"subscriber\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-event\"\n[events]\nsubscribe=[\"plugin.custom.thing\"]\n",
            riku_plugins::RIKU_PLUGIN_API
        ),
    )
    .unwrap();

    emit_as(
        &paths,
        plugin("emitter"),
        "plugin.custom.thing",
        r#"{"k":"v"}"#,
        Some("myapp"),
    )
    .unwrap();

    let body = std::fs::read_to_string(&received).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["event"], "plugin.custom.thing");
    assert_eq!(parsed["source_plugin"], "emitter");
    assert_eq!(parsed["app"], "myapp");
    assert_eq!(parsed["data"]["k"], "v");
}
