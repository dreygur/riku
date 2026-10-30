use super::*;
use crate::testkit::{put, rel};
use std::io::Read;
use std::os::unix::fs::symlink;

#[test]
fn bundle_holds_exactly_the_needed_files() {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "a.txt", "one");
    put(src.path(), "sub/b.txt", "two");
    put(src.path(), "skipped.txt", "three");
    let mut out = Vec::new();

    write_bundle(src.path(), &[rel("a.txt"), rel("sub/b.txt")], &mut out).unwrap();

    let mut tar_bytes = Vec::new();
    zstd::Decoder::new(out.as_slice())
        .unwrap()
        .read_to_end(&mut tar_bytes)
        .unwrap();
    let mut archive = tar::Archive::new(tar_bytes.as_slice());
    let mut found: Vec<(String, String)> = archive
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_string_lossy().into_owned();
            let mut body = String::new();
            entry.read_to_string(&mut body).unwrap();
            (name, body)
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            ("a.txt".to_string(), "one".to_string()),
            ("sub/b.txt".to_string(), "two".to_string())
        ]
    );
}

#[test]
fn bundle_refuses_a_symlink_in_need() {
    let src = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "secret", "s");
    symlink(outside.path().join("secret"), src.path().join("link")).unwrap();

    let err = write_bundle(src.path(), &[rel("link")], Vec::new()).unwrap_err();

    assert!(err.to_string().contains("link"), "error was: {err}");
}
