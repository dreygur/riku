use super::*;
use crate::testkit::{put, rel};
use std::io::Read;
use std::os::unix::fs::symlink;

/// Runs on every OS. Hole detection (SEEK_HOLE) only exists on Linux, Android
/// and FreeBSD, so elsewhere this passes trivially and Linux CI guards the bug.
#[test]
fn a_file_with_a_hole_is_bundled_as_a_regular_entry_and_applies() {
    use std::io::{Seek, SeekFrom, Write};
    let src = tempfile::tempdir().unwrap();
    let mut file = std::fs::File::create(src.path().join("db.img")).unwrap();
    file.set_len(4 << 20).unwrap();
    file.seek(SeekFrom::End(0)).unwrap();
    file.write_all(b"tail").unwrap();
    drop(file);
    let manifest = crate::scan::scan(src.path()).unwrap();
    let plan = crate::plan::plan_transfer(&src.path().join("none"), &manifest).unwrap();
    let mut out = Vec::new();
    write_bundle(src.path(), &plan.need, &mut out).unwrap();

    let mut tar_bytes = Vec::new();
    zstd::Decoder::new(out.as_slice())
        .unwrap()
        .read_to_end(&mut tar_bytes)
        .unwrap();
    let kinds: Vec<tar::EntryType> = tar::Archive::new(tar_bytes.as_slice())
        .entries()
        .unwrap()
        .map(|entry| entry.unwrap().header().entry_type())
        .collect();
    assert_eq!(kinds, [tar::EntryType::Regular]);
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");
    crate::apply::apply(&live, &manifest, &plan, out.as_slice()).unwrap();
    assert_eq!(crate::scan::scan(&live).unwrap(), manifest);
}

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
