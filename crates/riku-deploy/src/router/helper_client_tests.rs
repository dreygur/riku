use std::thread;

use nix::unistd::{Gid, Uid};
use riku_route::{bind_socket, serve_one};

use super::*;

fn own_uid() -> u32 {
    Uid::current().as_raw()
}

fn serve_status_once(
    deploy_uid: u32,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    thread::JoinHandle<()>,
) {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("router.sock");
    let listener = bind_socket(&socket, Gid::current().as_raw()).unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let _ = serve_one(stream, deploy_uid, &mut |_| Reply::Status {
            version: "test".to_string(),
        });
    });
    (dir, socket, server)
}

#[test]
fn status_reaches_the_helper_and_back() {
    let (_dir, socket, server) = serve_status_once(own_uid());

    let reply = send_to(&socket, &Request::Status {}, Duration::from_secs(5)).unwrap();

    server.join().unwrap();
    assert_eq!(
        reply,
        Reply::Status {
            version: "test".to_string()
        }
    );
}

#[test]
fn missing_socket_says_the_helper_is_not_running() {
    let dir = tempfile::tempdir().unwrap();

    let error = send_to(
        &dir.path().join("absent.sock"),
        &Request::Status {},
        Duration::from_secs(1),
    )
    .unwrap_err();

    assert!(error.to_string().contains("router helper"), "{error}");
}

#[test]
fn refused_peer_gets_an_error_not_a_reply() {
    if Uid::current().is_root() {
        return;
    }
    let (_dir, socket, server) = serve_status_once(own_uid() + 1);

    let outcome = send_to(&socket, &Request::Status {}, Duration::from_secs(5));

    server.join().unwrap();
    let error = outcome.unwrap_err().to_string();
    assert!(error.contains("without an answer"), "{error}");
}

#[test]
fn silent_helper_is_given_up_on() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("router.sock");
    let listener = bind_socket(&socket, Gid::current().as_raw()).unwrap();
    let silent = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        thread::sleep(Duration::from_millis(600));
        drop(stream);
    });

    let error = send_to(&socket, &Request::Status {}, Duration::from_millis(200)).unwrap_err();

    silent.join().unwrap();
    assert!(error.to_string().contains("timed out"), "{error}");
}
