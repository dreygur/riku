use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;

use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{fork, setuid, ForkResult, Gid, Uid};

use super::*;
use crate::bounded::MAX_MESSAGE_BYTES;
use crate::listen::bind_socket;
use crate::protocol::{encode_request, parse_reply};

struct Exchange {
    reply: Vec<u8>,
    served: Result<()>,
    handled: usize,
}

const NOBODY: u32 = 65534;

enum Client {
    Own,
    Nobody,
}

fn nobody_exchange(path: &Path, body: &[u8]) -> i32 {
    if setuid(Uid::from_raw(NOBODY)).is_err() {
        return 2;
    }
    let Ok(mut client) = UnixStream::connect(path) else {
        return 3;
    };
    let _ = client.write_all(body);
    let _ = client.shutdown(Shutdown::Write);
    let mut reply = Vec::new();
    let _ = client.read_to_end(&mut reply);
    i32::from(!reply.is_empty())
}

fn reply_as_nobody(path: &Path, body: &[u8]) -> Vec<u8> {
    // SAFETY: the child only makes system calls and plain allocations on its
    // own copy of the process, then leaves through _exit.
    match unsafe { fork() }.unwrap() {
        ForkResult::Child => unsafe { nix::libc::_exit(nobody_exchange(path, body)) },
        ForkResult::Parent { child } => match waitpid(child, None).unwrap() {
            WaitStatus::Exited(_, 0) => Vec::new(),
            WaitStatus::Exited(_, 1) => b"answered".to_vec(),
            other => panic!("client running as uid {NOBODY} failed: {other:?}"),
        },
    }
}

fn exchange(deploy_uid: u32, body: &[u8]) -> Exchange {
    exchange_with(deploy_uid, body, Client::Own)
}

fn exchange_with(deploy_uid: u32, body: &[u8], client: Client) -> Exchange {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("router.sock");
    let listener = bind_socket(&path, Gid::current().as_raw()).unwrap();
    if matches!(client, Client::Nobody) {
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    }
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut handled = 0;
        let mut handle = |request: Request| {
            handled += 1;
            match request {
                Request::Status {} => Reply::Status {
                    version: "test".to_string(),
                },
                _ => Reply::Ok { reloaded: false },
            }
        };
        let served = serve_one(stream, deploy_uid, &mut handle);
        (served, handled)
    });

    let reply = match client {
        Client::Own => {
            let mut stream = UnixStream::connect(&path).unwrap();
            let _ = stream.write_all(body);
            let _ = stream.shutdown(Shutdown::Write);
            let mut reply = Vec::new();
            let _ = stream.read_to_end(&mut reply);
            reply
        }
        Client::Nobody => reply_as_nobody(&path, body),
    };

    let (served, handled) = server.join().unwrap();
    Exchange {
        reply,
        served,
        handled,
    }
}

fn own_uid() -> u32 {
    Uid::current().as_raw()
}

fn status_body() -> Vec<u8> {
    encode_request(&Request::Status {}).unwrap()
}

#[test]
fn other_uid_is_refused() {
    assert!(peer_allowed(0, 1000), "root must be allowed");
    assert!(peer_allowed(1000, 1000), "the deploy user must be allowed");
    assert!(!peer_allowed(1001, 1000), "another user was allowed");
}

#[test]
fn status_round_trips() {
    let outcome = exchange(own_uid(), &status_body());

    assert!(outcome.served.is_ok(), "{:?}", outcome.served);
    assert_eq!(
        parse_reply(&outcome.reply).unwrap(),
        Reply::Status {
            version: "test".to_string()
        }
    );
}

#[test]
fn oversize_request_is_refused() {
    let mut body = vec![b' '; MAX_MESSAGE_BYTES];
    body.push(b' ');

    let outcome = exchange(own_uid(), &body);

    assert_eq!(
        outcome.handled, 0,
        "an oversize request reached the handler"
    );
    match parse_reply(&outcome.reply).unwrap() {
        Reply::Failed { stage, error } => {
            assert_eq!(stage, Stage::Schema);
            assert!(error.contains("larger than"), "unexpected error: {error}");
        }
        other => panic!("expected a schema failure, got {other:?}"),
    }
}

#[test]
fn request_at_the_size_limit_is_read() {
    let mut body = status_body();
    body.resize(MAX_MESSAGE_BYTES, b' ');

    let outcome = exchange(own_uid(), &body);

    assert_eq!(
        outcome.handled, 1,
        "a request of exactly the limit was refused"
    );
}

#[test]
fn unknown_field_gets_a_schema_failure() {
    let outcome = exchange(own_uid(), br#"{"v":1,"op":"status","path":"/etc/x"}"#);

    assert_eq!(
        outcome.handled, 0,
        "a request with an unknown field was handled"
    );
    assert!(matches!(
        parse_reply(&outcome.reply).unwrap(),
        Reply::Failed {
            stage: Stage::Schema,
            ..
        }
    ));
}

#[test]
fn connection_from_another_uid_gets_no_answer() {
    let outcome = if Uid::current().is_root() {
        exchange_with(1000, &status_body(), Client::Nobody)
    } else {
        exchange(own_uid() + 1, &status_body())
    };

    assert!(outcome.served.is_err(), "a foreign uid was served");
    assert_eq!(outcome.handled, 0, "a foreign uid reached the handler");
    assert!(
        outcome.reply.is_empty(),
        "a foreign uid got an answer: {:?}",
        String::from_utf8_lossy(&outcome.reply)
    );
}
