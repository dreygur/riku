use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn make_plugin(dir: &Path, name: &str, script: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn discover_skips_lifecycle_hooks() {
    let tmp = TempDir::new().unwrap();
    make_plugin(tmp.path(), "riku-pre-deploy", "#!/bin/sh\n");
    make_plugin(tmp.path(), "node", "#!/bin/sh\n");
    let plugins = discover(tmp.path());
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "node");
}

#[test]
fn discover_sorts_alphabetically() {
    let tmp = TempDir::new().unwrap();
    make_plugin(tmp.path(), "ruby", "#!/bin/sh\n");
    make_plugin(tmp.path(), "node", "#!/bin/sh\n");
    make_plugin(tmp.path(), "python", "#!/bin/sh\n");
    let plugins = discover(tmp.path());
    let names: Vec<_> = plugins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["node", "python", "ruby"]);
}

#[test]
fn discover_skips_non_executable() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("notexec");
    fs::write(&path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let plugins = discover(tmp.path());
    assert!(plugins.is_empty());
}

#[test]
fn detect_runtime_override_missing_plugin_errors() {
    let tmp = TempDir::new().unwrap();
    let plugins = discover(tmp.path());
    let mut env = HashMap::new();
    env.insert("RUNTIME".into(), "ghost".into());
    assert!(detect(&plugins, tmp.path(), &env).is_err());
}

#[test]
fn detect_runtime_override_selects_named_plugin() {
    let tmp = TempDir::new().unwrap();
    make_plugin(tmp.path(), "python", "#!/bin/sh\nexit 0\n");
    make_plugin(tmp.path(), "node", "#!/bin/sh\nexit 0\n");
    let plugins = discover(tmp.path());
    let mut env = HashMap::new();
    env.insert("RUNTIME".into(), "python".into());
    let result = detect(&plugins, tmp.path(), &env).unwrap();
    assert_eq!(result.unwrap().name, "python");
}

#[test]
fn detect_first_match_alphabetically() {
    let tmp = TempDir::new().unwrap();
    // Both accept — 'node' < 'python' alphabetically
    make_plugin(tmp.path(), "node", "#!/bin/sh\nexit 0\n");
    make_plugin(tmp.path(), "python", "#!/bin/sh\nexit 0\n");
    let plugins = discover(tmp.path());
    let result = detect(&plugins, tmp.path(), &HashMap::new())
        .unwrap()
        .unwrap();
    assert_eq!(result.name, "node");
}

#[test]
fn detect_returns_none_when_no_match() {
    let tmp = TempDir::new().unwrap();
    make_plugin(tmp.path(), "node", "#!/bin/sh\nexit 1\n");
    let plugins = discover(tmp.path());
    let result = detect(&plugins, tmp.path(), &HashMap::new()).unwrap();
    assert!(result.is_none());
}

#[test]
fn get_env_parses_key_value_lines() {
    let tmp = TempDir::new().unwrap();
    make_plugin(
        tmp.path(),
        "testplugin",
        "#!/bin/sh\necho 'FOO=bar'\necho '# comment'\necho ''\necho 'BAZ=qux'\n",
    );
    let plugins = discover(tmp.path());
    let ctx = RuntimeContext {
        app: "myapp",
        app_path: tmp.path(),
        env_path: tmp.path(),
        riku_root: tmp.path(),
        app_env: &HashMap::new(),
    };
    let env = get_env(&plugins[0], &ctx).unwrap();
    assert_eq!(env.get("FOO").map(String::as_str), Some("bar"));
    assert_eq!(env.get("BAZ").map(String::as_str), Some("qux"));
    assert_eq!(env.len(), 2);
}

#[test]
fn get_start_cmd_returns_first_nonempty_line() {
    let tmp = TempDir::new().unwrap();
    make_plugin(
        tmp.path(),
        "testplugin",
        "#!/bin/sh\necho ''\necho 'node server.js'\n",
    );
    let plugins = discover(tmp.path());
    let ctx = RuntimeContext {
        app: "myapp",
        app_path: tmp.path(),
        env_path: tmp.path(),
        riku_root: tmp.path(),
        app_env: &HashMap::new(),
    };
    let cmd = get_start_cmd(&plugins[0], &ctx).unwrap();
    assert_eq!(cmd.as_deref(), Some("node server.js"));
}

#[test]
fn parse_env_lines_handles_values_with_equals() {
    let raw = b"URL=http://example.com?foo=bar\nKEY=val\n";
    let env = parse_env_lines(raw).unwrap();
    assert_eq!(
        env.get("URL").map(String::as_str),
        Some("http://example.com?foo=bar")
    );
    assert_eq!(env.get("KEY").map(String::as_str), Some("val"));
}
