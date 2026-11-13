use std::os::unix::fs::PermissionsExt;

use riku_util::runtime_markers::BUNDLED_RUNTIMES;

use super::*;

#[test]
fn embedded_runtimes_plus_binaries_are_the_bundled_list() {
    let mut names: Vec<&str> = EMBEDDED_RUNTIMES.to_vec();
    names.extend(super::super::install_plugins::BINARY_PLUGINS);

    assert_eq!(names, BUNDLED_RUNTIMES);
    for name in EMBEDDED_RUNTIMES {
        assert!(find(name).is_some(), "{name} is not embedded");
    }
}

#[test]
fn bundled_plugins_print_plain_lines_for_the_deploy_rail() {
    for plugin in EMBEDDED {
        for file in plugin.files {
            let text = String::from_utf8_lossy(file.bytes);
            assert!(
                !text.contains("-----> "),
                "{}/{} prints a -----> prefix, which shows as `│  -----> ..` inside the rail",
                plugin.name,
                file.name
            );
        }
    }
}

/// Run the shipped python plugin's `build` in an app with `files`, with a
/// mock `uv` (when `with_uv`) that logs its arguments. Returns the exit
/// status, the uv log and stderr.
fn python_build(
    files: &[&str],
    version: Option<&str>,
    with_uv: bool,
    old_venv_version: Option<&str>,
) -> (bool, String, String) {
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    let bin = tmp.path().join("bin");
    let app = tmp.path().join("app");
    for dir in [&plugins, &bin, &app] {
        std::fs::create_dir_all(dir).unwrap();
    }
    write_out(find("python").unwrap(), &plugins).unwrap();
    if let Some(old) = old_venv_version {
        std::fs::create_dir_all(app.join(".venv")).unwrap();
        std::fs::write(app.join(".venv/.riku-python-version"), old).unwrap();
    }
    for file in files {
        std::fs::write(app.join(file), "").unwrap();
    }
    let log = tmp.path().join("uv.log");
    if with_uv {
        let uv = bin.join("uv");
        std::fs::write(
            &uv,
            format!("#!/bin/sh\necho \"$@\" >> {}\n", log.display()),
        )
        .unwrap();
        std::fs::set_permissions(&uv, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut command = std::process::Command::new(plugins.join("python"));
    command
        .arg("build")
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", tmp.path())
        .env("RIKU_APP_PATH", &app);
    if let Some(version) = version {
        command.env("RIKU_PYTHON_VERSION", version);
    }
    let output = command.output().unwrap();
    (
        output.status.success(),
        std::fs::read_to_string(&log).unwrap_or_default(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn python_version_reaches_uv_for_a_new_venv_and_a_locked_sync() {
    let (ok, venv_log, _) = python_build(&["requirements.txt"], Some("3.12"), true, None);
    let (locked_ok, sync_log, _) =
        python_build(&["pyproject.toml", "uv.lock"], Some("3.12"), true, None);

    assert!(ok && locked_ok);
    assert!(venv_log.contains("venv --python 3.12"), "{venv_log}");
    assert!(
        sync_log.contains("sync --python 3.12 --locked"),
        "{sync_log}"
    );
}

#[test]
fn no_python_version_leaves_uv_to_choose() {
    let (ok, log, _) = python_build(&["requirements.txt"], None, true, None);

    assert!(ok);
    assert!(!log.contains("--python"), "{log}");
}

#[test]
fn missing_python_version_without_uv_fails_naming_the_key() {
    let (ok, _, stderr) = python_build(&["requirements.txt"], Some("2.1"), false, None);

    assert!(!ok, "a build that cannot honour the version must fail");
    assert!(
        stderr.contains("RIKU_PYTHON_VERSION") && stderr.contains("python2.1"),
        "{stderr}"
    );
}

#[test]
fn changing_python_version_makes_the_venv_again() {
    let (ok, log, _) = python_build(&["requirements.txt"], Some("3.12"), true, Some("3.11"));
    let (kept_ok, kept_log, _) =
        python_build(&["requirements.txt"], Some("3.12"), true, Some("3.12"));

    assert!(ok && kept_ok);
    assert!(
        log.contains("venv --python 3.12"),
        "the 3.11 venv was kept: {log}"
    );
    assert!(
        !kept_log.contains("venv"),
        "a matching venv was made again: {kept_log}"
    );
}

#[test]
fn writing_node_needs_no_network() {
    let tmp = tempfile::tempdir().unwrap();

    write_out(find("node").unwrap(), tmp.path()).unwrap();

    let node = tmp.path().join("node");
    assert_eq!(
        std::fs::read(&node).unwrap(),
        include_bytes!("../../../../plugins/node")
    );
    assert_eq!(
        std::fs::metadata(&node).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[test]
fn riku_notify_keeps_its_bundle_layout() {
    let tmp = tempfile::tempdir().unwrap();

    write_out(find("riku-notify").unwrap(), tmp.path()).unwrap();

    let bundle = tmp.path().join("riku-notify");
    assert!(bundle.join("riku-plugin.toml").is_file());
    let entry = bundle.join("bin").join("on-event");
    assert_eq!(
        std::fs::metadata(&entry).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

/// Run the shipped node plugin's `build` with a mock `node` that reports
/// `installed` and, when `with_npm`, a mock `npm`. PATH holds only the
/// mocks (the plugin runs with `/bin/sh`), so a node, npm or nub on the host
/// cannot leak in. Returns the exit status and stderr.
fn node_build(wanted: Option<&str>, installed: &str, with_npm: bool) -> (bool, String) {
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    let bin = tmp.path().join("bin");
    let app = tmp.path().join("app");
    for dir in [&plugins, &bin, &app] {
        std::fs::create_dir_all(dir).unwrap();
    }
    write_out(find("node").unwrap(), &plugins).unwrap();
    std::fs::write(app.join("package.json"), "{}").unwrap();
    let mut tools = vec![("node", format!("echo v{installed}"))];
    if with_npm {
        tools.push(("npm", String::new()));
    }
    for (name, body) in tools {
        let tool = bin.join(name);
        std::fs::write(&tool, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut command = std::process::Command::new(plugins.join("node"));
    command
        .arg("build")
        .env_clear()
        .env("PATH", &bin)
        .env("HOME", tmp.path())
        .env("RIKU_APP_PATH", &app);
    if let Some(wanted) = wanted {
        command.env("RIKU_NODE_VERSION", wanted);
    }
    let output = command.output().unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn node_version_matches_on_whole_version_parts() {
    for wanted in ["22", "v22", "22.x", "22.11", "22.11.x", "22.11.1"] {
        let (ok, stderr) = node_build(Some(wanted), "22.11.1", true);
        assert!(ok, "{wanted} should match 22.11.1: {stderr}");
    }
    let (ok, _) = node_build(None, "18.0.0", true);
    assert!(ok, "no version set builds with any node");
}

#[test]
fn a_server_without_npm_or_nub_fails_the_build_saying_node_is_missing() {
    let (ok, stderr) = node_build(None, "22.11.1", false);

    assert!(!ok, "a build with no package manager must fail");
    assert!(
        stderr.contains("Node.js is not installed on this server (npm not found)"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("nub") && !stderr.contains("command not found"),
        "{stderr}"
    );
}

#[test]
fn python_go_and_ruby_builds_without_their_toolchain_say_it_is_missing() {
    for (plugin, marker, wanted) in [
        (
            "python",
            "requirements.txt",
            "Python is not installed on this server",
        ),
        ("go", "go.mod", "Go is not installed on this server"),
        ("ruby", "Gemfile", "Bundler is not installed on this server"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        let app = tmp.path().join("app");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&app).unwrap();
        write_out(find(plugin).unwrap(), tmp.path()).unwrap();
        std::fs::write(app.join(marker), "").unwrap();

        let output = std::process::Command::new(tmp.path().join(plugin))
            .arg("build")
            .env_clear()
            .env("PATH", &bin)
            .env("HOME", tmp.path())
            .env("RIKU_APP_PATH", &app)
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{plugin} built with no toolchain");
        assert!(stderr.contains(wanted), "{plugin}: {stderr}");
        assert!(!stderr.contains("command not found"), "{plugin}: {stderr}");
    }
}

#[test]
fn a_node_version_mismatch_fails_the_build_naming_both() {
    for wanted in ["20", "2", "22.1"] {
        let (ok, stderr) = node_build(Some(wanted), "22.11.1", true);

        assert!(!ok, "{wanted} must not match 22.11.1");
        assert!(
            stderr.contains("RIKU_NODE_VERSION") && stderr.contains("22.11.1"),
            "{stderr}"
        );
    }
}
