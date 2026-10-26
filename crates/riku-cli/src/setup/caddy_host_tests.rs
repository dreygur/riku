use super::*;

#[test]
fn import_added_once() {
    let line = import_line(Path::new("/home/deploy/.riku/caddy"));

    let updated = caddyfile_with_import(":80 {\n}\n", &line).expect("import should be added");
    assert!(
        updated.ends_with("}\nimport /home/deploy/.riku/caddy/*.caddy\n"),
        "{updated}"
    );
    assert_eq!(
        caddyfile_with_import(&updated, &line),
        None,
        "second run must not add it again"
    );
}
