use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::write_env_file;

fn env<const N: usize>(pairs: [(&str, &str); N]) -> BTreeMap<String, String> {
    pairs
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn writes_one_line_per_entry_readable_only_by_the_owner() {
    let dir = tempfile::tempdir().unwrap();

    let path = write_env_file(dir.path(), &env([("A", "1"), ("URL", "x=y z#w")])).unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "A=1\nURL=x=y z#w\n");
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn refuses_a_value_with_a_newline_naming_the_key() {
    let dir = tempfile::tempdir().unwrap();

    let err = write_env_file(dir.path(), &env([("KEY", "a\n--privileged")]))
        .unwrap_err()
        .to_string();

    assert!(err.contains("KEY"), "{err}");
    assert!(!err.contains("privileged"), "value echoed: {err}");
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "file left behind"
    );
}

#[test]
fn refuses_a_key_with_equals_naming_the_key() {
    let dir = tempfile::tempdir().unwrap();

    let err = write_env_file(dir.path(), &env([("A=B", "1")]))
        .unwrap_err()
        .to_string();

    assert!(err.contains("A=B"), "{err}");
}

#[test]
fn refuses_empty_whitespace_and_comment_keys() {
    let dir = tempfile::tempdir().unwrap();
    for key in ["", "A B", "A\tB", "#A"] {
        assert!(
            write_env_file(dir.path(), &env([(key, "1")])).is_err(),
            "key {key:?} was accepted"
        );
    }
}
