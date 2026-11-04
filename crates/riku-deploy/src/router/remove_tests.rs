use std::path::{Path, PathBuf};

use crate::config::{ReloadPrograms, RikuPaths};

use super::super::reload::mock_systemctl::{mock_systemctl, write_script, Run};

const OLD: u64 = 100;
const NEW: u64 = 1 << 62;

fn paths_with(dir: &Path, systemctl: PathBuf) -> RikuPaths {
    let mut paths = RikuPaths::for_tests(dir);
    std::fs::create_dir_all(&paths.nginx_root).unwrap();
    crate::nginx::write_include_file(&paths).unwrap();
    paths.reload_programs = ReloadPrograms {
        systemctl,
        caddy: write_script(dir, "mock_caddy", "exit 0\n"),
        nginx: write_script(
            dir,
            "mock_nginx",
            &format!("touch {}/direct\n", dir.display()),
        ),
    };
    paths
}

#[test]
fn removing_a_destroyed_app_passes_the_guard_and_reloads_through_the_unit() {
    let tmp = tempfile::tempdir().unwrap();
    let phases: &[&[Run]] = &[&[(NEW, NEW + 1, 0, "inactive")]];
    let paths = paths_with(tmp.path(), mock_systemctl(tmp.path(), phases));

    // No app directory: destroy deletes it before it removes the route.
    std::fs::write(paths.nginx_root.join("myapp.conf"), "server {}\n").unwrap();

    let outcome = crate::router::remove("myapp", &paths);

    assert!(outcome.is_ok(), "remove failed: {outcome:?}");
    assert!(
        !tmp.path().join("direct").exists(),
        "direct nginx reload was invoked instead of waiting on the unit"
    );
}

#[test]
fn retried_removal_after_a_failed_reload_reloads_again() {
    let tmp = tempfile::tempdir().unwrap();
    let phases: &[&[Run]] = &[&[(50, OLD, 0, "inactive"), (NEW, NEW + 1, 1, "inactive")]];
    let paths = paths_with(tmp.path(), mock_systemctl(tmp.path(), phases));
    let error_file = paths.riku_root.join(super::super::RELOAD_ERROR_FILE);
    std::fs::write(&error_file, "earlier reload failed\n").unwrap();

    let outcome = crate::router::remove("myapp", &paths);

    assert!(
        outcome.is_err(),
        "the removal must reload and report the failing unit: {outcome:?}"
    );
    let recorded = std::fs::read_to_string(&error_file).unwrap_or_default();
    assert!(
        recorded.contains("riku-nginx-reload.service"),
        "the error file must hold this reload's failure, got {recorded:?}"
    );
}
