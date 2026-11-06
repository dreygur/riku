use std::fs;

use super::*;

#[test]
fn include_names_etc_riku_and_replaces_the_old_user_include() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    assert!(!includes_riku_sites(&paths), "no include file yet");
    fs::create_dir_all(paths.nginx_include_file.parent().unwrap()).unwrap();
    fs::write(
        &paths.nginx_include_file,
        format!("include {}/*.conf;\n", paths.nginx_root.display()),
    )
    .unwrap();
    assert!(!includes_riku_sites(&paths), "old include counted");

    write_include_file(&paths, false).unwrap();

    let contents = fs::read_to_string(&paths.nginx_include_file).unwrap();
    assert!(
        contents.contains("include /etc/riku/nginx/*.conf;"),
        "{contents}"
    );
    assert!(
        !contents.contains(&paths.nginx_root.display().to_string()),
        "the old include survived: {contents}"
    );
    assert!(includes_riku_sites(&paths));
}
