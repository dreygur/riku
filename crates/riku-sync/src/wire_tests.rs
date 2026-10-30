use super::*;

#[test]
fn rel_path_refuses_escapes() {
    for bad in [
        "",
        "/etc/passwd",
        "../x",
        "a/../b",
        "a//b",
        "a/./b",
        "a\\b",
        "a/",
        "a\0b",
    ] {
        assert!(RelPath::try_from(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn rel_path_parent_and_to_path() {
    let path = RelPath::try_from("venv/bin/python").unwrap();
    assert_eq!(path.to_path(), std::path::PathBuf::from("venv/bin/python"));
    assert_eq!(path.parent().unwrap().as_str(), "venv/bin");
    assert!(RelPath::try_from("app.py").unwrap().parent().is_none());
}

#[test]
fn manifest_json_with_escape_fails_because_of_the_path() {
    let raw = r#"{"entries":[{"path":"../x","kind":"dir"}]}"#;
    let err = serde_json::from_str::<Manifest>(raw)
        .unwrap_err()
        .to_string();
    assert!(err.contains("../x"), "error does not name the path: {err}");

    let clean = r#"{"entries":[{"path":"x","kind":"dir"}]}"#;
    let manifest: Manifest = serde_json::from_str(clean).unwrap();
    assert_eq!(manifest.entries[0].path.as_str(), "x");
    assert_eq!(manifest.entries[0].kind, EntryKind::Dir);
}

#[test]
fn file_entry_round_trips_in_the_flat_wire_shape() {
    let raw = r#"{"path":"app.py","kind":"file","size":6,"sha256":"ab","mode":420}"#;
    let entry: Entry = serde_json::from_str(raw).unwrap();
    assert_eq!(
        entry.kind,
        EntryKind::File {
            size: 6,
            sha256: "ab".into(),
            mode: 0o644
        }
    );
    let back: serde_json::Value = serde_json::to_value(&entry).unwrap();
    assert_eq!(
        back,
        serde_json::from_str::<serde_json::Value>(raw).unwrap()
    );
}
