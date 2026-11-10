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

    let hidden: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| entry.file_name())
        .collect();
    assert!(hidden.is_empty(), "temp files left behind: {hidden:?}");
}

// The CLI and the supervisor's install route can write one plugin at once.
#[test]
fn concurrent_writes_of_one_plugin_all_succeed() {
    let tmp = tempfile::tempdir().unwrap();
    let body = vec![b'x'; 256 * 1024];
    let failures: Vec<String> = std::thread::scope(|scope| {
        let writers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    (0..20)
                        .filter_map(|_| write_plugin_file(tmp.path(), "node", &body, 0o755).err())
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        writers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    assert!(
        failures.is_empty(),
        "{} concurrent writes failed, first: {:?}",
        failures.len(),
        failures.first()
    );
    assert_eq!(fs::read(tmp.path().join("node")).unwrap(), body);
}
