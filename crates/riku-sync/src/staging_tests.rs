use super::*;
use crate::testkit::{file_entry, manifest_of, put};
use std::collections::HashSet;

#[test]
fn staging_with_an_unlisted_file_is_refused() {
    let parent = tempfile::tempdir().unwrap();
    let staging = Staging::create(parent.path().join("new")).unwrap();
    put(&parent.path().join("new"), "listed.txt", "a");
    put(&parent.path().join("new"), "sneaked.txt", "b");
    let manifest = manifest_of(vec![file_entry("listed.txt", "a")]);
    let listed: HashSet<&str> = manifest.entries.iter().map(|e| e.path.as_str()).collect();

    let err = staging.ensure_only(&listed).unwrap_err();

    assert!(err.to_string().contains("sneaked.txt"), "error was: {err}");
}
