use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::*;

fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let script = dir.join(name);
    std::fs::write(&script, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

/// A `systemctl` that answers each call with the next "timestamp status"
/// line of `runs`, repeating the last line once they run out.
fn mock_systemctl(dir: &Path, runs: &[(u64, u32)]) -> PathBuf {
    let sequence: String = runs
        .iter()
        .map(|(timestamp, status)| format!("{timestamp} {status}\n"))
        .collect();
    std::fs::write(dir.join("sequence"), sequence).unwrap();
    let body = format!(
        "dir={dir}\n\
         calls=$(cat \"$dir/calls\" 2>/dev/null || echo 0)\n\
         calls=$((calls + 1))\n\
         echo $calls > \"$dir/calls\"\n\
         line=$(sed -n \"${{calls}}p\" \"$dir/sequence\")\n\
         [ -n \"$line\" ] || line=$(tail -n 1 \"$dir/sequence\")\n\
         set -- $line\n\
         echo ExecMainExitTimestampMonotonic=$1\n\
         echo ExecMainStatus=$2\n\
         echo LoadState=loaded\n",
        dir = dir.display(),
    );
    write_script(dir, "mock_systemctl", &body)
}

fn tools_with(dir: &Path, systemctl: PathBuf, caddy: &str) -> ReloadTools {
    ReloadTools {
        systemctl,
        caddy: write_script(dir, "mock_caddy", caddy),
        nginx: dir.join("mock_nginx_unused"),
    }
}

fn nginx_reload(runs: &[(u64, u32)]) -> (anyhow::Result<()>, RikuPaths, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let tools = tools_with(tmp.path(), mock_systemctl(tmp.path(), runs), "exit 0\n");
    let baseline = Baseline::capture(&Router::Nginx, &tools);
    let outcome = reload_with(&tools, &Router::Nginx, &paths, &baseline);
    (outcome, paths, tmp)
}

#[test]
fn nginx_reload_waits_for_newer_unit_run() {
    let (outcome, paths, _tmp) = nginx_reload(&[(100, 0), (100, 0), (200, 0)]);

    outcome.unwrap();
    assert!(!paths.riku_root.join(ERROR_FILE).exists());
}

#[test]
fn nginx_reload_fails_on_unit_error() {
    let (outcome, paths, _tmp) = nginx_reload(&[(100, 0), (200, 1)]);

    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("riku-nginx-reload.service"), "{err}");
    let recorded = std::fs::read_to_string(paths.riku_root.join(ERROR_FILE)).unwrap();
    assert_eq!(recorded, err);
}

#[test]
fn nginx_reload_times_out() {
    let (outcome, _paths, _tmp) = nginx_reload(&[(100, 0)]);

    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("did not confirm"), "{err}");
}

#[test]
fn caddy_reload_reports_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let tools = tools_with(
        tmp.path(),
        tmp.path().join("mock_systemctl_unused"),
        "echo 'admin endpoint disabled' >&2\nexit 1\n",
    );

    let err = reload_with(&tools, &Router::Caddy, &paths, &Baseline::Direct)
        .unwrap_err()
        .to_string();

    assert!(err.contains("admin endpoint disabled"), "{err}");
}
