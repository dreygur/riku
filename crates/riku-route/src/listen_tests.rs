use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use nix::unistd::Gid;

use super::*;

#[test]
fn activation_needs_our_pid_and_one_descriptor() {
    assert_eq!(activated_fd(Some("42"), Some("1"), 42).unwrap(), Some(3));
    assert_eq!(activated_fd(Some("41"), Some("1"), 42).unwrap(), None);
    assert_eq!(activated_fd(None, Some("1"), 42).unwrap(), None);
    assert!(activated_fd(Some("42"), Some("2"), 42).is_err());
    assert!(activated_fd(Some("42"), None, 42).is_err());
}

#[test]
fn live_socket_is_not_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("router.sock");
    let gid = Gid::current().as_raw();
    let _first = bind_socket(&path, gid).unwrap();

    let error = bind_socket(&path, gid).unwrap_err().to_string();

    assert!(error.contains("another router helper"), "{error}");
}

#[test]
fn socket_is_created_group_writable_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("router.sock");

    let _listener = bind_socket(&path, Gid::current().as_raw()).unwrap();

    let mode = fs::symlink_metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o660, "socket mode is {mode:o}");
}

#[test]
fn stale_socket_is_replaced_but_a_regular_file_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("router.sock");
    drop(UnixListener::bind(&socket).unwrap());
    let file = dir.path().join("notes.txt");
    fs::write(&file, "keep").unwrap();
    let gid = Gid::current().as_raw();

    assert!(
        bind_socket(&socket, gid).is_ok(),
        "a stale socket blocked the bind"
    );
    assert!(
        bind_socket(&file, gid).is_err(),
        "a regular file was replaced"
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), "keep");
}

#[test]
fn idle_listener_returns_after_the_idle_time() {
    let dir = tempfile::tempdir().unwrap();
    let listener = bind_socket(&dir.path().join("router.sock"), Gid::current().as_raw()).unwrap();
    let started = Instant::now();

    let outcome = run(&listener, Some(Duration::from_millis(100)), 0, &mut |_| {
        Reply::Ok { reloaded: false }
    });

    assert!(outcome.is_ok(), "{outcome:?}");
    assert!(started.elapsed() >= Duration::from_millis(100));
}
