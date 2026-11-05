use std::os::unix::fs::PermissionsExt;

use nix::sys::socket::{bind, socket, AddressFamily, SockFlag, SockType, UnixAddr};
use nix::sys::wait::waitpid;
use nix::unistd::{fork, ForkResult, Gid};

use super::*;

// A socket bound in this process can still accept a connect after it is
// dropped, while a parallel test's fork holds a copy of the descriptor. Bind
// it in a child that exits, so no process here ever holds it.
fn leave_stale_socket(path: &std::path::Path) {
    let addr = UnixAddr::new(path).unwrap();
    match unsafe { fork() }.unwrap() {
        ForkResult::Child => {
            let bound = socket(
                AddressFamily::Unix,
                SockType::Stream,
                SockFlag::empty(),
                None,
            )
            .and_then(|fd| bind(std::os::fd::AsRawFd::as_raw_fd(&fd), &addr));
            unsafe { nix::libc::_exit(if bound.is_ok() { 0 } else { 1 }) }
        }
        ForkResult::Parent { child } => {
            waitpid(child, None).unwrap();
        }
    }
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
    leave_stale_socket(&socket);
    assert!(socket.exists(), "the child did not leave a socket");
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
