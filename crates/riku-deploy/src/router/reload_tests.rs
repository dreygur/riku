use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::config::ReloadPrograms;

fn paths_with_caddy(dir: &std::path::Path, body: &str) -> RikuPaths {
    let script = dir.join("mock_caddy");
    std::fs::write(&script, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut paths = RikuPaths::for_tests(dir);
    paths.reload_programs = ReloadPrograms {
        caddy: script,
        ..ReloadPrograms::default()
    };
    paths
}

#[test]
fn caddy_reload_reports_stderr_and_records_it_for_doctor() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_caddy(tmp.path(), "echo 'admin endpoint disabled' >&2\nexit 1\n");

    let err = reload_caddy(&paths).unwrap_err().to_string();

    assert!(err.contains("admin endpoint disabled"), "{err}");
    let recorded = std::fs::read_to_string(paths.riku_root.join(ERROR_FILE)).unwrap();
    assert_eq!(recorded, err);
}

#[test]
fn a_confirmed_reload_clears_the_recorded_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_caddy(tmp.path(), "exit 0\n");
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(paths.riku_root.join(ERROR_FILE), "earlier failure").unwrap();

    reload_caddy(&paths).unwrap();

    assert!(!paths.riku_root.join(ERROR_FILE).exists());
}
