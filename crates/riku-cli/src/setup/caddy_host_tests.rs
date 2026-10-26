use std::path::PathBuf;

use super::*;

#[test]
fn import_added_once() {
    let home = PathBuf::from("/home/deploy");
    let line = import_line(&RikuPaths::from_dirs(home.join(".riku"), &home));

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
