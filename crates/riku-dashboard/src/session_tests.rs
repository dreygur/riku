use std::io::Write;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};

use super::*;

#[test]
fn a_pipe_or_socket_on_stdin_is_a_session_to_follow() {
    let mut child = Command::new("true").stdout(Stdio::piped()).spawn().unwrap();
    let pipe = File::from(OwnedFd::from(child.stdout.take().unwrap()));
    child.wait().unwrap();
    let (socket, _peer) = UnixStream::pair().unwrap();

    assert!(follows_session(&pipe), "a pipe, as sshd gives a command");
    assert!(
        follows_session(&File::from(OwnedFd::from(socket))),
        "a socket"
    );
}

/// systemd and the `&` in the `rk dashboard` script give /dev/null; a
/// redirected file is no session either.
#[test]
fn dev_null_or_a_file_on_stdin_is_no_session() {
    let file = tempfile::tempfile().unwrap();

    assert!(!follows_session(&File::open("/dev/null").unwrap()));
    assert!(!follows_session(&file));
}

#[test]
fn drain_returns_once_the_writer_closes() {
    let (mut writer, reader) = UnixStream::pair().unwrap();
    writer
        .write_all(b"anything the client sends is ignored\n")
        .unwrap();
    drop(writer);

    drain(reader);
}
