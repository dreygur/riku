use std::fs;

use nix::sys::stat::Mode;

use super::*;

#[test]
fn reads_a_positive_pid_and_nothing_else() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join(PID_FILE);
    assert_eq!(probe(&path), None, "no file is no pid file");

    for (content, want) in [
        ("12345\n", Some(12345)),
        ("", None),
        ("not-a-number\n", None),
        // `kill(0, ..)` signals our process group, `kill(-1, ..)` everything.
        ("0\n", None),
        ("-1\n", None),
    ] {
        fs::write(&path, content).unwrap();
        assert_eq!(
            probe(&path).map(|file| file.pid),
            Some(want),
            "pid file {content:?}"
        );
    }
}

/// The pid of a live process (this test's own) in a pid file nobody locks
/// is what a dead supervisor leaves once its pid is reused.
#[test]
fn only_a_locked_pid_file_names_a_running_supervisor() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join(PID_FILE);
    fs::write(&path, std::process::id().to_string()).unwrap();

    assert_eq!(running_pid(&path), None);

    let held = fs::File::open(&path).unwrap();
    held.lock().unwrap();
    assert_eq!(running_pid(&path), Some(std::process::id() as i32));
}

/// The deploy user controls this path and root reads it: a link would make
/// root read another file, and a FIFO would hang it.
#[test]
fn a_symlink_or_fifo_pid_file_is_refused_without_blocking() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("elsewhere");
    fs::write(&target, "4242\n").unwrap();
    let link = tmp.path().join("link.pid");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let fifo = tmp.path().join("fifo.pid");
    nix::unistd::mkfifo(&fifo, Mode::from_bits_truncate(0o600)).unwrap();

    assert_eq!(
        probe(&link),
        None,
        "a symlinked pid file must not be followed"
    );
    assert_eq!(probe(&fifo), None, "a FIFO is not a pid file");
}
