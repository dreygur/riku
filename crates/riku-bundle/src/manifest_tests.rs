use super::*;

fn valid_toml() -> String {
    format!(
        r#"
        name = "slack-notify"
        version = "0.1.0"
        type = "notifier"
        api = {RIKU_PLUGIN_API}
        entry = "bin/notify"
        [events]
        subscribe = ["deploy.finished", "deploy.failed"]
        mode = "observe"
        "#
    )
}

#[test]
fn parses_a_valid_manifest() {
    let m = PluginManifest::from_toml_str(&valid_toml()).unwrap();
    assert_eq!(m.name, "slack-notify");
    assert_eq!(m.plugin_type, PluginType::Notifier);
    assert!(m.subscribes_to("deploy.finished"));
    assert!(!m.subscribes_to("build.started"));
}

#[test]
fn rejects_unsupported_api() {
    let toml = "name=\"n\"\nversion=\"1\"\ntype=\"addon\"\napi=999\nentry=\"x\"\n";
    assert!(PluginManifest::from_toml_str(toml).is_err());
}

#[test]
fn rejects_entry_path_traversal() {
    let toml = format!(
        "name=\"n\"\nversion=\"1\"\ntype=\"addon\"\napi={RIKU_PLUGIN_API}\nentry=\"../../etc/evil\"\n"
    );
    let err = PluginManifest::from_toml_str(&toml)
        .unwrap_err()
        .to_string();
    assert!(err.contains("relative path"), "got: {err}");
}

#[test]
fn rejects_absolute_entry() {
    let toml = format!(
        "name=\"n\"\nversion=\"1\"\ntype=\"addon\"\napi={RIKU_PLUGIN_API}\nentry=\"/usr/bin/x\"\n"
    );
    assert!(PluginManifest::from_toml_str(&toml).is_err());
}

#[test]
fn rejects_unsafe_name() {
    let toml = format!(
        "name=\"../evil\"\nversion=\"1\"\ntype=\"addon\"\napi={RIKU_PLUGIN_API}\nentry=\"x\"\n"
    );
    assert!(PluginManifest::from_toml_str(&toml).is_err());
}

#[test]
fn entry_path_joins_under_bundle() {
    let m = PluginManifest::from_toml_str(&valid_toml()).unwrap();
    assert_eq!(
        m.entry_path(Path::new("/plugins/slack")),
        Path::new("/plugins/slack/bin/notify")
    );
}

#[test]
fn refuses_any_event_mode_but_observe() {
    let toml = format!(
        "name=\"n\"\nversion=\"1\"\ntype=\"hook\"\napi={RIKU_PLUGIN_API}\nentry=\"x\"\n[events]\nsubscribe=[\"deploy.started\"]\nmode=\"gate\"\n"
    );
    let err = PluginManifest::from_toml_str(&toml)
        .expect_err("only observe mode may install")
        .to_string();
    assert!(
        err.contains("only supported mode is \"observe\""),
        "got: {err}"
    );
}
