use std::fs;
use std::path::Path;

use super::*;

#[test]
fn include_line_refuses_a_directory_that_could_end_the_directive() {
    let mut paths = RikuPaths::for_tests(Path::new("/srv"));
    for bad in ["/srv/a;b", "/srv/a\nevents {}", "/srv/a b", "relative/riku"] {
        paths.nginx_root = Path::new(bad).to_path_buf();
        assert!(
            include_line(&paths).is_err(),
            "accepted nginx directory {bad:?}"
        );
    }
}

#[test]
fn include_file_is_recognised_once_written() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    assert!(!includes_riku_sites(&paths), "no include file yet");

    write_include_file(&paths).unwrap();

    let contents = fs::read_to_string(&paths.nginx_include_file).unwrap();
    assert!(
        contents.contains(&format!("include {}/*.conf;", paths.nginx_root.display())),
        "{contents}"
    );
    assert!(includes_riku_sites(&paths));
}
