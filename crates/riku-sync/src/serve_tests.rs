use crate::lock;
use crate::session_kit::{
    changed_source, error_reported_to_client, honest_client, refused, refused_to,
};
use crate::testkit::{fixture, raw_bundle};
use crate::MAX_LINE_BYTES;
use std::io::{self, BufReader, Cursor, Read, Write};

#[test]
fn cut_stream_leaves_live_untouched() {
    let f = fixture();
    let src = changed_source();
    let (request, bundle) = honest_client(src.path(), &f.live);
    let mut input = request;
    input.extend_from_slice(&bundle[..bundle.len() / 2]);
    let (err, reply) = refused(&f, Cursor::new(input));
    assert!(
        err.contains("bundle stream is damaged") && err.contains("incomplete frame"),
        "server error was: {err}"
    );
    assert_eq!(
        error_reported_to_client(&reply),
        "{\"error\":\"the bundle was refused\"}"
    );
}

#[test]
fn second_session_while_the_lock_is_held_gets_the_busy_text() {
    let f = fixture();
    let src = changed_source();
    let (request, _) = honest_client(src.path(), &f.live);
    let _running = lock::acquire(&f.live).unwrap();
    let (err, reply) = refused(&f, Cursor::new(request));
    assert_eq!(
        reply.lines().count(),
        1,
        "a plan was sent before the lock was checked: {reply}"
    );
    assert_eq!(
        error_reported_to_client(&reply),
        "{\"error\":\"another sync of this app is in progress\"}"
    );
    assert!(err.contains("another sync"), "server error was: {err}");
}

#[test]
fn first_line_that_is_not_json_is_refused() {
    let f = fixture();
    let (_, reply) = refused(&f, Cursor::new(b"this is not json\n".to_vec()));
    assert_eq!(
        error_reported_to_client(&reply),
        "{\"error\":\"the first line is not a sync request\"}"
    );
}

struct GoneClient;

impl Write for GoneClient {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn client_vanishing_before_the_plan_is_written_is_refused() {
    let f = fixture();
    let src = changed_source();
    let (request, _) = honest_client(src.path(), &f.live);
    let err = refused_to(&f, Cursor::new(request), GoneClient);
    assert!(err.contains("cannot write to the peer"), "got: {err}");
}

struct FailingStream;

impl Read for FailingStream {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("os detail from the server"))
    }
}

#[test]
fn read_failure_is_reported_with_a_fixed_text() {
    let f = fixture();
    let (err, reply) = refused(&f, BufReader::new(FailingStream));
    assert!(err.contains("os detail"), "cause lost: {err}");
    assert_eq!(
        error_reported_to_client(&reply),
        "{\"error\":\"the manifest line could not be read\"}"
    );
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
    let (err, reply) = refused(&f, Cursor::new(input));
    assert!(
        err.contains("bundle stream is damaged") && err.contains("Unknown frame descriptor"),
        "server error was: {err}"
    );
    assert_eq!(
        error_reported_to_client(&reply),
        "{\"error\":\"the bundle was refused\"}"
    );
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
