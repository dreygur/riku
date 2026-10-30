//! Helpers shared by the session, send and serve tests.

use crate::testkit::{names_in, put, Fixture, SETTLED};
use crate::{plan_transfer, receive, scan, write_bundle, Manifest};
use std::io::{BufRead, Write};
use std::path::Path;

pub fn request_line(manifest: &Manifest) -> Vec<u8> {
    let mut line =
        serde_json::to_vec(&serde_json::json!({"sync": 1, "manifest": manifest})).unwrap();
    line.push(b'\n');
    line
}

/// The request line and the bundle an honest client would send for `src`
/// against the fixture's live tree.
pub fn honest_client(src: &Path, live: &Path) -> (Vec<u8>, Vec<u8>) {
    let manifest = scan(src).unwrap();
    let plan = plan_transfer(live, &manifest).unwrap();
    let mut bundle = Vec::new();
    write_bundle(src, &plan.need, &mut bundle).unwrap();
    (request_line(&manifest), bundle)
}

pub fn changed_source() -> tempfile::TempDir {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "keep.txt", "k");
    put(src.path(), "new.txt", "n");
    put(src.path(), "dir/inner.txt", "CHANGED");
    src
}

/// Runs `receive` on `input` writing to `out`, checks it refused and left live
/// and its parent as they were, and returns the error text.
pub fn refused_to(f: &Fixture, input: impl BufRead, out: impl Write) -> String {
    let before = scan(&f.live).unwrap();
    let err = receive(&f.live, input, out)
        .expect_err("receive must refuse")
        .to_string();
    assert_eq!(scan(&f.live).unwrap(), before, "live changed: {err}");
    for name in names_in(f.parent.path()) {
        assert!(
            SETTLED.contains(&name.as_str()),
            "stray {name:?} after: {err}"
        );
    }
    err
}

/// Like [`refused_to`], collecting what the client was sent.
pub fn refused(f: &Fixture, input: impl BufRead) -> (String, String) {
    let mut reply = Vec::new();
    let err = refused_to(f, input, &mut reply);
    (err, String::from_utf8(reply).unwrap())
}

pub fn error_reported_to_client(reply: &str) -> &str {
    let last = reply.lines().last().unwrap_or("");
    assert!(
        last.starts_with("{\"error\":"),
        "last line sent was: {last:?}"
    );
    last
}
