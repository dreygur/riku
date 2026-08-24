use super::*;
use std::os::unix::fs::PermissionsExt;

fn write_exec(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn make_bus_paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    (tmp, paths)
}

fn write_filter_bundle(bundle: &Path, name: &str, filter_name: &str, priority: i32, script: &str) {
    std::fs::create_dir_all(bundle.join("bin")).unwrap();
    write_exec(&bundle.join("bin/on-filter"), script);
    std::fs::write(
            bundle.join("riku-plugin.toml"),
            format!(
                "name=\"{name}\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-filter\"\n[filters]\nsubscribe=[\"{filter_name}\"]\npriority={priority}\n",
                crate::RIKU_PLUGIN_API
            ),
        )
        .unwrap();
}

#[test]
fn no_subscribers_returns_input_unchanged() {
    let (_tmp, paths) = make_bus_paths();
    let result = FilterBus::new(&paths).apply("nginx.include_content", serde_json::json!(""));
    assert_eq!(result, serde_json::json!(""));
}

#[test]
fn single_filter_transforms_the_value() {
    let (_tmp, paths) = make_bus_paths();
    let bundle = paths.plugin_root.join("uppercaser");
    write_filter_bundle(
            &bundle,
            "uppercaser",
            "greeting",
            0,
            "#!/bin/sh\nread line\ndata=$(printf '%s' \"$line\" | sed 's/.*\"data\":\"\\([^\"]*\\)\".*/\\1/')\nupper=$(printf '%s' \"$data\" | tr a-z A-Z)\nprintf '{\"data\":\"%s\"}' \"$upper\"\n",
        );

    let result = FilterBus::new(&paths).apply("greeting", serde_json::json!("hello"));
    assert_eq!(result, serde_json::json!("HELLO"));
}

#[test]
fn chain_runs_in_priority_order_each_seeing_previous_output() {
    let (_tmp, paths) = make_bus_paths();

    // "second" installed first / alphabetically first, but priority 5
    // means it must run AFTER "first" (priority 1): proves ordering
    // isn't filesystem or name order.
    write_filter_bundle(
            &paths.plugin_root.join("second"),
            "second",
            "chain",
            5,
            "#!/bin/sh\nread line\ndata=$(printf '%s' \"$line\" | sed 's/.*\"data\":\"\\([^\"]*\\)\".*/\\1/')\nprintf '{\"data\":\"%sB\"}' \"$data\"\n",
        );
    write_filter_bundle(
            &paths.plugin_root.join("first"),
            "first",
            "chain",
            1,
            "#!/bin/sh\nread line\ndata=$(printf '%s' \"$line\" | sed 's/.*\"data\":\"\\([^\"]*\\)\".*/\\1/')\nprintf '{\"data\":\"%sA\"}' \"$data\"\n",
        );

    let result = FilterBus::new(&paths).apply("chain", serde_json::json!("x"));
    assert_eq!(result, serde_json::json!("xAB"));
}

#[test]
fn broken_filter_degrades_to_passthrough_not_failure() {
    let (_tmp, paths) = make_bus_paths();
    let bundle = paths.plugin_root.join("broken");
    write_filter_bundle(&bundle, "broken", "chain", 0, "#!/bin/sh\nexit 1\n");

    let result = FilterBus::new(&paths).apply("chain", serde_json::json!("unchanged"));
    assert_eq!(result, serde_json::json!("unchanged"));
}

#[test]
fn malformed_output_degrades_to_passthrough() {
    let (_tmp, paths) = make_bus_paths();
    let bundle = paths.plugin_root.join("malformed");
    write_filter_bundle(
        &bundle,
        "malformed",
        "chain",
        0,
        "#!/bin/sh\necho 'not json'\n",
    );

    let result = FilterBus::new(&paths).apply("chain", serde_json::json!("unchanged"));
    assert_eq!(result, serde_json::json!("unchanged"));
}
