use super::*;
use crate::testkit::{fixture, names_in, put, raw_bundle, rel, Fixture, SETTLED};
use crate::{plan_transfer, receive, scan, write_bundle, Manifest};
use std::fs;
use std::io::{self, BufRead, BufReader, Cursor, Read};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;

fn request_line(manifest: &Manifest) -> Vec<u8> {
    let mut line =
        serde_json::to_vec(&serde_json::json!({"sync": 1, "manifest": manifest})).unwrap();
    line.push(b'\n');
    line
}

/// The request line and the bundle an honest client would send for `src`
/// against the fixture's live tree.
fn honest_client(src: &Path, live: &Path) -> (Vec<u8>, Vec<u8>) {
    let manifest = scan(src).unwrap();
    let plan = plan_transfer(live, &manifest).unwrap();
    let mut bundle = Vec::new();
    write_bundle(src, &plan.need, &mut bundle).unwrap();
    (request_line(&manifest), bundle)
}

fn changed_source() -> tempfile::TempDir {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "keep.txt", "k");
    put(src.path(), "new.txt", "n");
    put(src.path(), "dir/inner.txt", "CHANGED");
    src
}

/// Runs `receive` on `input`, checks it refused and left live and its parent as
/// they were, and returns the error text and everything sent to the client.
fn refused(f: &Fixture, input: impl BufRead) -> (String, String) {
    let before = scan(&f.live).unwrap();
    let mut reply = Vec::new();
    let err = receive(&f.live, input, &mut reply)
        .expect_err("receive must refuse")
        .to_string();
    assert_eq!(scan(&f.live).unwrap(), before, "live changed: {err}");
    for name in names_in(f.parent.path()) {
        assert!(
            SETTLED.contains(&name.as_str()),
            "stray {name:?} after: {err}"
        );
    }
    (err, String::from_utf8(reply).unwrap())
}

fn error_reported_to_client(reply: &str) -> &str {
    let last = reply.lines().last().unwrap_or("");
    assert!(
        last.starts_with("{\"error\":"),
        "last line sent was: {last:?}"
    );
    last
}

fn sync_over_sockets(src: &Path, live: &Path) -> anyhow::Result<SendReport> {
    let (client_out, server_in) = UnixStream::pair().unwrap();
    let (server_out, client_in) = UnixStream::pair().unwrap();
    let live = live.to_path_buf();
    let server = thread::spawn(move || receive(&live, BufReader::new(server_in), server_out));
    let report = send(src, BufReader::new(client_in), client_out);
    let served = server.join().unwrap();
    assert!(served.is_ok(), "server side failed: {served:?}");
    report
}

#[test]
fn second_sync_moves_only_the_changed_file() {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "app.py", "v1");
    put(src.path(), "old.txt", "x");
    put(src.path(), "lib/util.py", "u");
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");

    let first = sync_over_sockets(src.path(), &live).unwrap();
    assert_eq!(
        first.sent,
        vec![rel("app.py"), rel("lib/util.py"), rel("old.txt")],
        "first sync must send every file"
    );
    assert!(first.bytes > 0, "no bytes counted for the first bundle");

    put(src.path(), "app.py", "v2");
    fs::remove_file(src.path().join("old.txt")).unwrap();
    let second = sync_over_sockets(src.path(), &live).unwrap();
    assert_eq!(second.sent, vec![rel("app.py")]);
    assert_eq!(second.deleted, vec![rel("old.txt")]);
    assert_eq!(scan(&live).unwrap(), scan(src.path()).unwrap());
}

#[test]
fn cut_stream_leaves_live_untouched() {
    let f = fixture();
    let src = changed_source();
    let (request, bundle) = honest_client(src.path(), &f.live);
    let mut input = request;
    input.extend_from_slice(&bundle[..bundle.len() / 2]);
    let (err, reply) = refused(&f, Cursor::new(input));
    error_reported_to_client(&reply);
    assert!(!err.is_empty());
}

#[test]
fn unknown_protocol_is_refused() {
    let f = fixture();
    let line = b"{\"sync\":2,\"manifest\":{\"entries\":[]}}\n".to_vec();
    let (err, reply) = refused(&f, Cursor::new(line));
    assert!(
        error_reported_to_client(&reply).contains("sync protocol 2"),
        "client was told: {reply}"
    );
    assert!(err.contains("sync protocol 2"), "server error was: {err}");
}

#[test]
fn overlong_manifest_line_is_refused() {
    let f = fixture();
    let flood = BufReader::new(io::repeat(b'a').take(MAX_LINE_BYTES as u64 + 1));
    let (err, reply) = refused(&f, flood);
    let cap = MAX_LINE_BYTES.to_string();
    assert!(err.contains(&cap), "error must name the cap: {err}");
    assert!(
        error_reported_to_client(&reply).contains(&cap),
        "client was told: {reply}"
    );
}

#[test]
fn closed_stream_before_the_manifest_is_refused() {
    let f = fixture();
    let (_, reply) = refused(&f, Cursor::new(Vec::new()));
    error_reported_to_client(&reply);
}

#[test]
fn manifest_with_escaping_path_is_refused_without_echoing_it() {
    let f = fixture();
    let line = b"{\"sync\":1,\"manifest\":{\"entries\":[{\"path\":\"../../etc/passwd\",\"kind\":\"dir\"}]}}\n";
    let (_, reply) = refused(&f, Cursor::new(line.to_vec()));
    let told = error_reported_to_client(&reply);
    assert!(
        !told.contains("passwd") && !told.contains(".."),
        "peer supplied path was echoed back: {told}"
    );
}

#[test]
fn garbage_where_the_bundle_belongs_is_refused() {
    let f = fixture();
    let src = changed_source();
    let (request, _) = honest_client(src.path(), &f.live);
    let mut input = request;
    input.extend_from_slice(b"this is not a zstd stream");
    let (_, reply) = refused(&f, Cursor::new(input));
    error_reported_to_client(&reply);
}

#[test]
fn bundle_with_an_unrequested_file_is_refused() {
    let f = fixture();
    let src = changed_source();
    let (request, _) = honest_client(src.path(), &f.live);
    let mut input = request;
    input.extend(raw_bundle(&[
        ("dir/inner.txt", b"CHANGED"),
        ("new.txt", b"n"),
        ("evil.txt", b"x"),
    ]));
    let (err, reply) = refused(&f, Cursor::new(input));
    assert!(err.contains("not requested"), "server error was: {err}");
    assert!(
        !error_reported_to_client(&reply).contains("evil"),
        "peer supplied name was echoed back: {reply}"
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
fn sender_refuses_a_plan_for_a_file_it_never_listed() {
    let (outcome, sent) = send_against(Cursor::new(
        b"{\"need\":[\"secret.txt\"],\"delete\":[]}\n".to_vec(),
    ));
    let err = outcome.unwrap_err().to_string();
    assert!(err.contains("not a file in the manifest"), "got: {err}");
    assert!(only_the_request_was_sent(&sent), "a bundle was sent anyway");
}

#[test]
fn sender_fails_when_the_server_closes_before_the_plan() {
    let (outcome, _) = send_against(Cursor::new(Vec::new()));
    assert!(outcome.is_err(), "no plan arrived but send succeeded");
}

#[test]
fn sender_fails_on_a_garbage_plan() {
    let (outcome, sent) = send_against(Cursor::new(b"not json at all\n".to_vec()));
    assert!(outcome.is_err(), "garbage plan accepted");
    assert!(only_the_request_was_sent(&sent), "a bundle was sent anyway");
}

#[test]
fn sender_refuses_an_overlong_plan_reply() {
    let flood = BufReader::new(io::repeat(b'a').take(MAX_LINE_BYTES as u64 + 1));
    let (outcome, _) = send_against(flood);
    let err = outcome.unwrap_err().to_string();
    assert!(err.contains(&MAX_LINE_BYTES.to_string()), "got: {err}");
}

#[test]
fn sender_returns_the_server_error_text_without_control_characters() {
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
fn sender_fails_when_the_server_never_confirms() {
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
