use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::*;

const OLD: u64 = 100;
const NEW: u64 = 1 << 62;

/// One `systemctl show` answer: start, exit, status, ActiveState.
type Run = (u64, u64, u32, &'static str);

fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let script = dir.join(name);
    std::fs::write(&script, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

/// Pin a modification time in whole seconds. The mock shell compares times
/// with `-nt`, which on macOS only looks at seconds.
fn set_mtime(path: &Path, epoch_secs: u64) {
    let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(epoch_secs);
    std::fs::File::open(path)
        .and_then(|handle| handle.set_modified(when))
        .unwrap();
}

fn lines(runs: &[Run]) -> String {
    runs.iter()
        .map(|(start, exit, status, active)| format!("{start} {exit} {status} {active}\n"))
        .collect()
}

/// A `systemctl` that answers each call with the next line of `runs`,
/// repeating the last once they run out. Once the nginx config directory
/// has been touched after the mock was written, it answers with `touched`.
fn mock_systemctl(dir: &Path, runs: &[Run], touched: &[Run]) -> PathBuf {
    std::fs::write(dir.join("sequence"), lines(runs)).unwrap();
    std::fs::write(dir.join("touched_sequence"), lines(touched)).unwrap();
    let watched = dir.join(".riku/nginx");
    std::fs::create_dir_all(&watched).unwrap();
    std::fs::write(dir.join("marker"), "").unwrap();
    set_mtime(&watched, 1_000_000_000);
    set_mtime(&dir.join("marker"), 1_100_000_000);
    let body = format!(
        "dir={dir}\n\
         calls=$(cat \"$dir/calls\" 2>/dev/null || echo 0)\n\
         calls=$((calls + 1))\n\
         echo $calls > \"$dir/calls\"\n\
         seq=\"$dir/sequence\"\n\
         if [ \"$dir/.riku/nginx\" -nt \"$dir/marker\" ] && [ -s \"$dir/touched_sequence\" ]; then seq=\"$dir/touched_sequence\"; fi\n\
         line=$(sed -n \"${{calls}}p\" \"$seq\")\n\
         [ -n \"$line\" ] || line=$(tail -n 1 \"$seq\")\n\
         set -- $line\n\
         echo ExecMainStartTimestampMonotonic=$1\n\
         echo ExecMainExitTimestampMonotonic=$2\n\
         echo ExecMainStatus=$3\n\
         echo ActiveState=$4\n\
         echo LoadState=loaded\n",
        dir = dir.display(),
    );
    write_script(dir, "mock_systemctl", &body)
}

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

fn nginx_reload(
    runs: &[Run],
    touched: &[Run],
) -> (anyhow::Result<()>, RikuPaths, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with(
        tmp.path(),
        mock_systemctl(tmp.path(), runs, touched),
        "exit 0\n",
    );
    let baseline = Baseline::capture(&paths.reload_programs);
    let outcome = reload(&Router::Nginx, &paths, &baseline);
    (outcome, paths, tmp)
}

#[test]
fn nginx_reload_waits_for_a_run_started_after_the_write() {
    let (outcome, paths, _tmp) = nginx_reload(
        &[
            (50, OLD, 0, "inactive"),
            (50, OLD, 0, "inactive"),
            (NEW, NEW + 1, 0, "inactive"),
        ],
        &[],
    );

    outcome.unwrap();
    assert!(!paths.riku_root.join(ERROR_FILE).exists());
}

#[test]
fn nginx_reload_ignores_a_run_that_started_before_the_write() {
    let (outcome, _paths, _tmp) = nginx_reload(
        &[
            (50, OLD, 0, "inactive"),
            (150, OLD, 0, "activating"),
            (150, 160, 0, "inactive"),
            (NEW, NEW + 1, 0, "inactive"),
        ],
        &[],
    );

    outcome.unwrap();
}

#[test]
fn nginx_reload_fails_when_only_the_old_run_finishes() {
    let (outcome, _paths, _tmp) = nginx_reload(
        &[(150, OLD, 0, "activating"), (150, 160, 0, "inactive")],
        &[],
    );

    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("did not confirm"), "{err}");
}

#[test]
fn nginx_reload_retriggers_the_path_unit_once_the_old_run_ends() {
    let (outcome, _paths, _tmp) = nginx_reload(
        &[(150, OLD, 0, "activating"), (150, 160, 0, "inactive")],
        &[(NEW, NEW + 1, 0, "inactive")],
    );

    outcome.unwrap();
}

#[test]
fn nginx_reload_fails_on_unit_error() {
    let (outcome, paths, _tmp) = nginx_reload(
        &[(50, OLD, 0, "inactive"), (NEW, NEW + 1, 1, "inactive")],
        &[],
    );

    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("riku-nginx-reload.service"), "{err}");
    let recorded = std::fs::read_to_string(paths.riku_root.join(ERROR_FILE)).unwrap();
    assert_eq!(recorded, err);
}

#[test]
fn nginx_reload_times_out() {
    let (outcome, _paths, _tmp) = nginx_reload(&[(50, OLD, 0, "inactive")], &[]);

    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("did not confirm"), "{err}");
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
