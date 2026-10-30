use super::*;
use crate::testkit::put;
use crate::MAX_LINE_BYTES;
use std::io::{BufReader, Cursor, Read};
use std::os::unix::net::UnixStream;
use std::thread;

fn incompressible(len: usize) -> Vec<u8> {
    let mut state: u64 = 12345;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 56) as u8
        })
        .collect()
}

#[test]
fn prefers_the_servers_refusal_over_a_broken_pipe_mid_bundle() {
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("big.bin"), incompressible(4 << 20)).unwrap();
    let (client_out, server_in) = UnixStream::pair().unwrap();
    let (mut server_out, client_in) = UnixStream::pair().unwrap();
    let server = thread::spawn(move || {
        let mut from_client = BufReader::new(server_in);
        let mut request = String::new();
        from_client.read_line(&mut request).unwrap();
        server_out
            .write_all(b"{\"need\":[\"big.bin\"],\"delete\":[]}\n")
            .unwrap();
        server_out
            .write_all(b"{\"error\":\"the bundle was refused\"}\n")
            .unwrap();
    });
    let outcome = send(src.path(), BufReader::new(client_in), client_out);
    server.join().unwrap();
    let err = outcome.unwrap_err().to_string();
    assert!(
        err.contains("the bundle was refused"),
        "the server's text was masked by: {err}"
    );
}

fn send_against(server_says: impl BufRead) -> (anyhow::Result<SendReport>, Vec<u8>) {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "app.py", "v1");
    let mut sent = Vec::new();
    let outcome = send(src.path(), server_says, &mut sent);
    (outcome, sent)
}

fn only_the_request_was_sent(sent: &[u8]) -> bool {
    sent.iter().filter(|byte| **byte == b'\n').count() == 1 && sent.ends_with(b"\n")
}

#[test]
fn refuses_a_plan_for_a_file_it_never_listed() {
    let (outcome, sent) = send_against(Cursor::new(
        b"{\"need\":[\"secret.txt\"],\"delete\":[]}\n".to_vec(),
    ));
    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("not a file in the manifest"), "got: {err}");
    assert!(only_the_request_was_sent(&sent), "a bundle was sent anyway");
}

#[test]
fn fails_when_the_server_closes_before_the_plan() {
    let (outcome, _) = send_against(Cursor::new(Vec::new()));
    assert!(outcome.is_err(), "no plan arrived but send succeeded");
}

#[test]
fn fails_on_a_garbage_plan() {
    let (outcome, sent) = send_against(Cursor::new(b"not json at all\n".to_vec()));
    assert!(outcome.is_err(), "garbage plan accepted");
    assert!(only_the_request_was_sent(&sent), "a bundle was sent anyway");
}

#[test]
fn refuses_an_overlong_plan_reply() {
    let flood = BufReader::new(io::repeat(b'a').take(MAX_LINE_BYTES as u64 + 1));
    let (outcome, _) = send_against(flood);
    let err = outcome.unwrap_err().to_string();
    assert!(err.contains(&MAX_LINE_BYTES.to_string()), "got: {err}");
}

#[test]
fn returns_the_server_error_text_without_control_characters() {
    let (outcome, _) = send_against(Cursor::new(
        b"{\"error\":\"disk \\u001b[31mfull\"}\n".to_vec(),
    ));
    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("disk"), "server text missing: {err}");
    assert!(
        !err.contains('\u{1b}'),
        "escape byte reached the terminal: {err:?}"
    );
}

#[test]
fn fails_when_the_server_never_confirms() {
    let (outcome, sent) = send_against(Cursor::new(
        b"{\"need\":[\"app.py\"],\"delete\":[]}\n".to_vec(),
    ));
    assert!(
        outcome.is_err(),
        "no confirmation arrived but send succeeded"
    );
    assert!(
        !only_the_request_was_sent(&sent),
        "the bundle was never written"
    );
}
