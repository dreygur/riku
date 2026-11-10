use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn writes_the_file_with_its_final_mode() {
    let tmp = tempfile::tempdir().unwrap();

    write_plugin_file(tmp.path(), "node", b"#!/bin/sh\n", 0o755).unwrap();

    let mode = fs::metadata(tmp.path().join("node"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o755, "node is {mode:o}");
    assert_eq!(fs::read(tmp.path().join("node")).unwrap(), b"#!/bin/sh\n");
}

#[test]
fn a_failed_write_leaves_no_temp_file() {
    let tmp = tempfile::tempdir().unwrap();
    // A non empty directory where the plugin goes makes the final rename fail.
    fs::create_dir_all(tmp.path().join("node").join("keep")).unwrap();

    assert!(write_plugin_file(tmp.path(), "node", b"#!/bin/sh\n", 0o755).is_err());

    assert!(
        !tmp.path().join(".node.new").exists(),
        "a temp file was left behind"
    );
}
