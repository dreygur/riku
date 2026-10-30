use super::*;

fn signed_manifest(original: &str, signatures: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(MANIFEST_FILE), original).unwrap();
    for signature in signatures {
        write_manifest_signature(dir.path(), signature).unwrap();
    }
    fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap()
}

#[test]
fn signature_goes_before_the_first_table() {
    let out = signed_manifest("name = \"a\"\n\n[capabilities]\nnetwork = true\n", &["aa"]);
    assert_eq!(
        out,
        "name = \"a\"\n\nsignature = \"aa\"\n[capabilities]\nnetwork = true\n"
    );
}

#[test]
fn signature_is_appended_when_there_is_no_table() {
    let out = signed_manifest("name = \"a\"\n", &["aa"]);
    assert_eq!(out, "name = \"a\"\nsignature = \"aa\"\n");
}

#[test]
fn resigning_replaces_the_old_signature_and_keeps_similar_keys() {
    let out = signed_manifest(
        "name = \"a\"\nsignature_note = \"keep\"\nsignature = \"old\"\n[x]\n",
        &["new"],
    );
    assert_eq!(
        out,
        "name = \"a\"\nsignature_note = \"keep\"\nsignature = \"new\"\n[x]\n"
    );
    let twice = signed_manifest("name = \"a\"\n[x]\n", &["one", "two"]);
    assert_eq!(twice.matches("signature").count(), 1, "{twice}");
}
