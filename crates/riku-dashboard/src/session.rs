//! Ending the dashboard with the ssh session that started it.
//!
//! sshd sends SIGHUP only to a session with a pty. A non-interactive
//! `ssh riku@host dashboard ...` gets none when the client disconnects, so
//! the dashboard kept running and held its port. What it does see is its
//! stdin pipe reaching end of file and its parent, sshd's session process,
//! going away. When stdin is a pipe or a socket the dashboard stops on
//! either. Any other stdin means no session to follow: a terminal gets its
//! SIGHUP, and `/dev/null` is what a systemd unit gives, and what the `&` in
//! the `rk dashboard` script gives (that script stops the dashboard itself).

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::os::fd::AsFd;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::process::parent_id;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tokio::sync::Notify;

/// How often the parent pid is compared with the one at start.
const PARENT_POLL: Duration = Duration::from_secs(1);

/// Starts following the session when stdin is a pipe or a socket. The
/// returned `Notify` fires once stdin reaches end of file or the parent
/// process changes. `None` when there is no session to follow.
pub(crate) fn watch() -> Option<Arc<Notify>> {
    let stdin = match std::io::stdin().as_fd().try_clone_to_owned() {
        Ok(fd) => File::from(fd),
        Err(_) => return None,
    };
    if !follows_session(&stdin) {
        return None;
    }
    let ended = Arc::new(Notify::new());
    let on_eof = Arc::clone(&ended);
    thread::spawn(move || {
        drain(stdin);
        on_eof.notify_one();
    });
    let on_orphan = Arc::clone(&ended);
    let parent = parent_id();
    thread::spawn(move || {
        while parent_id() == parent {
            thread::sleep(PARENT_POLL);
        }
        on_orphan.notify_one();
    });
    Some(ended)
}

/// Whether `stdin` is a pipe or a socket, which is what sshd gives a
/// command it runs.
fn follows_session(stdin: &File) -> bool {
    match stdin.metadata() {
        Ok(meta) => {
            let kind = meta.file_type();
            kind.is_fifo() || kind.is_socket()
        }
        Err(_) => false,
    }
}

/// Reads and drops `input` until end of file or a read error.
fn drain(mut input: impl Read) {
    let mut buf = [0u8; 512];
    loop {
        match input.read(&mut buf) {
            Ok(0) => return,
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
