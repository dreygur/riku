use std::fs;
use std::process::Command;
use std::time::Duration;

use super::*;

/// One enabled worker config of `myapp`, as a deploy leaves it.
fn deployed_worker(paths: &RikuPaths) -> PathBuf {
    fs::create_dir_all(&paths.workers_available).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    let available = paths.workers_available.join("myapp@worker.1.toml");
    fs::write(&available, "[worker]\n").unwrap();
    let enabled = paths.workers_enabled.join("myapp@worker.1.toml");
    std::os::unix::fs::symlink(&available, &enabled).unwrap();
    enabled
}

/// A supervisor that died and left its pid file, the pid since taken by an
/// unrelated live process. Nobody holds the file's lock.
#[test]
fn stale_pid_file_is_not_a_running_supervisor() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let enabled = deployed_worker(&paths);
    let mut unrelated = Command::new("sleep").arg("30").spawn().unwrap();
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(
        paths.riku_root.join("supervisor.pid"),
        unrelated.id().to_string(),
    )
    .unwrap();

    let restarted = restart_configs_with(&[enabled], &paths, |_| false);

    // A SIGHUP, had one been sent, ends `sleep` well within this.
    std::thread::sleep(Duration::from_millis(200));
    let signalled = unrelated.try_wait().unwrap();
    let _ = unrelated.kill();
    let _ = unrelated.wait();
    let err = restarted.expect_err("with no supervisor running the restart must fail");
    assert!(
        format!("{err:#}").contains("supervisor is not running"),
        "the error must say why nothing restarted: {err:#}"
    );
    assert_eq!(
        signalled, None,
        "the process now holding the stale pid must not be signalled"
    );
}
