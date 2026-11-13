use std::fs;
use std::process::Command;
use std::time::Duration;

use super::*;
use crate::fake_supervisor::FakeSupervisor;

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

fn hour_ago(path: &std::path::Path) -> std::time::SystemTime {
    let then = std::time::SystemTime::now() - Duration::from_secs(3600);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(then)
        .unwrap();
    fs::metadata(path).unwrap().modified().unwrap()
}

/// An enabled entry may only lead to a config file in `workers_available`:
/// a link to anything else would let a restart touch a file it names.
#[test]
fn restart_refuses_an_enabled_link_out_of_workers_available() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    deployed_worker(&paths);
    let outside = tmp.path().join("outside.txt");
    fs::write(&outside, "not a worker config").unwrap();
    let before = hour_ago(&outside);
    let planted = paths.workers_enabled.join("myapp@worker.2.toml");
    std::os::unix::fs::symlink(&outside, &planted).unwrap();
    let _supervisor = FakeSupervisor::start(&paths);

    let restarted = restart_configs(&[planted], &paths);

    assert!(
        restarted.is_err(),
        "a link out of workers_available must be refused"
    );
    assert_eq!(
        fs::metadata(&outside).unwrap().modified().unwrap(),
        before,
        "the file outside workers_available must keep its mtime"
    );
}

/// A FIFO opened for writing with no reader blocks forever; the restart
/// must refuse it without opening it that way.
#[test]
fn restart_refuses_a_fifo_config_without_blocking() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    deployed_worker(&paths);
    let fifo = paths.workers_available.join("myapp@worker.2.toml");
    nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::from_bits_truncate(0o600)).unwrap();
    let enabled = paths.workers_enabled.join("myapp@worker.2.toml");
    std::os::unix::fs::symlink(&fifo, &enabled).unwrap();
    let _supervisor = FakeSupervisor::start(&paths);

    let restarted = restart_configs(&[enabled], &paths);

    let err = restarted.expect_err("a FIFO is not a worker config");
    assert!(
        format!("{err:#}").contains("not a regular file"),
        "the error must say why: {err:#}"
    );
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
