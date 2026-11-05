use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::thread;

use nix::unistd::{Gid, Uid};

use super::*;
use crate::bounded::MAX_MESSAGE_BYTES;
use crate::listen::bind_socket;
use crate::protocol::{encode_request, parse_reply};

struct Exchange {
    reply: Vec<u8>,
    served: Result<()>,
    handled: usize,
}

fn exchange(deploy_uid: u32, body: &[u8]) -> Exchange {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("router.sock");
    let listener = bind_socket(&path, Gid::current().as_raw()).unwrap();
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

    let mut stream = UnixStream::connect(&path).unwrap();
    let _ = stream.write_all(body);
    let _ = stream.shutdown(Shutdown::Write);
    let mut reply = Vec::new();
    let _ = stream.read_to_end(&mut reply);

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
fn connection_from_another_uid_gets_no_answer() {
    let outcome = exchange(own_uid() + 1, &status_body());

    assert!(outcome.served.is_err(), "a foreign uid was served");
    assert_eq!(outcome.handled, 0, "a foreign uid reached the handler");
    assert!(
        outcome.reply.is_empty(),
        "a foreign uid got an answer: {:?}",
        String::from_utf8_lossy(&outcome.reply)
    );
}
