//! The node runtime plugin on a server with nub and no `node`: the plugin
//! runs against a fake `nub` that pins, provisions and resolves like the
//! real one (https://nubjs.com/docs/node), with a PATH that holds no node.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Tools the plugin and the fake nub call, linked into the test's PATH so
/// a node installed on the machine running the tests stays out of reach.
const TOOLS: [&str; 6] = ["sh", "cat", "dirname", "mktemp", "rm", "mkdir"];

const FAKE_NUB: &str = r#"#!/bin/sh
case "$1 $2" in
  "node pin") printf '%s\n' "$3" > .node-version ;;
  "node install") [ -f .node-version ] ;;
  "node which")
    [ -f .node-version ] || exit 1
    printf '%s/%s/bin/node\n' "$FAKE_NODES" "$(cat .node-version)"
    ;;
  install*) printf 'nub %s\n' "$*" ;;
  *) exit 1 ;;
esac
"#;

fn write_script(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn which(tool: &str) -> PathBuf {
    let out = Command::new("sh")
        .args(["-c", &format!("command -v {tool}")])
        .output()
        .unwrap();
    PathBuf::from(String::from_utf8(out.stdout).unwrap().trim())
}

struct NubServer {
    _root: tempfile::TempDir,
    bin: PathBuf,
    nodes: PathBuf,
    app: PathBuf,
}

/// A PATH with nub and basic tools only, and fake node builds for `lts`
/// (26.13.1) and `24` (24.22.3) where the fake nub resolves them.
fn nub_server() -> NubServer {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    for tool in TOOLS {
        std::os::unix::fs::symlink(which(tool), bin.join(tool)).unwrap();
    }
    write_script(&bin.join("nub"), FAKE_NUB);
    let nodes = root.path().join("nodes");
    for (pin, version) in [("lts", "v26.13.1"), ("24", "v24.22.3")] {
        let node = format!("#!/bin/sh\necho {version}\n");
        write_script(&nodes.join(pin).join("bin/node"), &node);
    }
    let app = root.path().join("app");
    fs::create_dir_all(&app).unwrap();
    fs::write(app.join("package.json"), "{\"main\": \"server.js\"}\n").unwrap();
    NubServer {
        _root: root,
        bin,
        nodes,
        app,
    }
}

fn plugin(server: &NubServer, verb: &str, node_version: Option<&str>) -> Output {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/node");
    let mut cmd = Command::new(server.bin.join("sh"));
    cmd.arg(script)
        .arg(verb)
        .env_clear()
        .env("PATH", &server.bin)
        .env("FAKE_NODES", &server.nodes)
        .env("RIKU_APP_PATH", &server.app);
    if let Some(version) = node_version {
        cmd.env("RIKU_NODE_VERSION", version);
    }
    cmd.output().unwrap()
}

fn path_line(server: &NubServer, node_version: Option<&str>) -> String {
    let out = plugin(server, "env", node_version);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("PATH="))
        .unwrap_or_else(|| panic!("no PATH line in env output:\n{stdout}"))
        .to_string()
}

#[test]
fn unpinned_app_runs_on_the_lts_node_nub_provides() {
    let server = nub_server();

    let build = plugin(&server, "build", None);
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let lts_dir = server.nodes.join("lts/bin");
    let path = path_line(&server, None);
    assert!(
        path.starts_with(&format!(
            "{}/node_modules/.bin:{}:",
            server.app.display(),
            lts_dir.display()
        )),
        "the app's processes cannot find node: PATH={path}"
    );
    assert!(
        !server.app.join(".node-version").exists(),
        "the plugin pinned inside the app"
    );
}

#[test]
fn riku_node_version_picks_the_node_nub_provides() {
    let server = nub_server();

    let build = plugin(&server, "build", Some("24"));
    assert!(
        build.status.success(),
        "the version check saw the wrong node: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let path = path_line(&server, Some("24"));
    assert!(
        path.contains(&format!("{}:", server.nodes.join("24/bin").display())),
        "PATH={path}"
    );
}
