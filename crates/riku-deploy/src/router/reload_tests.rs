use std::path::{Path, PathBuf};

use super::mock_systemctl::{mock_systemctl, write_script, Run};
use super::*;

const OLD: u64 = 100;
const NEW: u64 = 1 << 62;

fn paths_with(dir: &Path, systemctl: PathBuf, caddy: &str) -> RikuPaths {
    let mut paths = RikuPaths::for_tests(dir);
    std::fs::create_dir_all(&paths.nginx_root).unwrap();
    paths.reload_programs = ReloadPrograms {
        systemctl,
        caddy: write_script(dir, "mock_caddy", caddy),
        nginx: write_script(
            dir,
            "mock_nginx",
            &format!("touch {}/direct\n", dir.display()),
        ),
    };
    paths
}

fn nginx_reload(phases: &[&[Run]]) -> (anyhow::Result<()>, RikuPaths, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with(tmp.path(), mock_systemctl(tmp.path(), phases), "exit 0\n");
    let baseline = Baseline::capture(&paths.reload_programs);
    let outcome = reload(&Router::Nginx, &paths, &baseline);
    (outcome, paths, tmp)
}

#[test]
fn nginx_reload_waits_for_a_run_started_after_the_write() {
    let (outcome, paths, _tmp) = nginx_reload(&[&[
        (50, OLD, 0, "inactive"),
        (50, OLD, 0, "inactive"),
        (NEW, NEW + 1, 0, "inactive"),
    ]]);

    outcome.unwrap();
    assert!(!paths.riku_root.join(ERROR_FILE).exists());
}

#[test]
fn nginx_reload_ignores_a_run_that_started_before_the_write() {
    let (outcome, _paths, _tmp) = nginx_reload(&[&[
        (50, OLD, 0, "inactive"),
        (150, OLD, 0, "activating"),
        (150, 160, 0, "inactive"),
        (NEW, NEW + 1, 0, "inactive"),
    ]]);

    outcome.unwrap();
}

#[test]
fn nginx_reload_fails_when_only_the_old_run_finishes() {
    let (outcome, _paths, _tmp) =
        nginx_reload(&[&[(150, OLD, 0, "activating"), (150, 160, 0, "inactive")]]);

    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("did not confirm"), "{err}");
}

#[test]
fn nginx_reload_retriggers_the_path_unit_once_the_old_run_ends() {
    let (outcome, _paths, _tmp) = nginx_reload(&[
        &[(150, OLD, 0, "activating"), (150, 160, 0, "inactive")],
        &[(NEW, NEW + 1, 0, "inactive")],
    ]);

    outcome.unwrap();
}

#[test]
fn nginx_reload_retriggers_after_a_run_that_started_unseen_and_finished() {
    let (outcome, _paths, _tmp) = nginx_reload(&[
        &[(50, OLD, 0, "inactive"), (150, 160, 0, "inactive")],
        &[(NEW, NEW + 1, 0, "inactive")],
    ]);

    outcome.unwrap();
}

#[test]
fn nginx_reload_fails_on_unit_error() {
    let (outcome, paths, _tmp) =
        nginx_reload(&[&[(50, OLD, 0, "inactive"), (NEW, NEW + 1, 1, "inactive")]]);

    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("riku-nginx-reload.service"), "{err}");
    let recorded = std::fs::read_to_string(paths.riku_root.join(ERROR_FILE)).unwrap();
    assert_eq!(recorded, err);
}

#[test]
fn nginx_reload_times_out() {
    let (outcome, _paths, _tmp) = nginx_reload(&[&[(50, OLD, 0, "inactive")]]);

    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("did not confirm"), "{err}");
}

/// A trigger that lands while another deploy's run is active is lost, so the
/// wait has to touch again once the unit is idle.
#[test]
fn nginx_reload_retriggers_again_when_a_trigger_is_lost() {
    let (outcome, _paths, _tmp) = nginx_reload(&[
        &[(50, OLD, 0, "inactive")],
        &[(150, OLD, 0, "active"), (150, 160, 0, "inactive")],
        &[(NEW, NEW + 1, 0, "inactive")],
    ]);

    outcome.unwrap();
}

#[test]
fn nginx_without_a_usable_unit_is_reloaded_directly() {
    let broken = [
        ("not-found", "echo LoadState=not-found\n"),
        ("empty output", "exit 0\n"),
        ("systemctl exit 1", "echo LoadState=loaded\nexit 1\n"),
    ];
    for (case, script) in broken {
        let tmp = tempfile::tempdir().unwrap();
        let mut paths = paths_with(tmp.path(), PathBuf::new(), "exit 0\n");
        paths.reload_programs.systemctl = write_script(tmp.path(), "mock_systemctl", script);

        let baseline = Baseline::capture(&paths.reload_programs);
        assert!(matches!(baseline, Baseline::Direct), "{case}");
        reload(&Router::Nginx, &paths, &baseline).unwrap();
        assert!(
            tmp.path().join("direct").exists(),
            "{case}: nginx was not run"
        );
    }
}

#[test]
fn caddy_reload_reports_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with(
        tmp.path(),
        tmp.path().join("mock_systemctl_unused"),
        "echo 'admin endpoint disabled' >&2\nexit 1\n",
    );

    let err = reload(&Router::Caddy, &paths, &Baseline::Direct)
        .unwrap_err()
        .to_string();

    assert!(err.contains("admin endpoint disabled"), "{err}");
}
