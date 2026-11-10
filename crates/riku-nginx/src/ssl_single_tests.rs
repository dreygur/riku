use super::*;

fn paths() -> (tempfile::TempDir, crate::config::RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = crate::config::RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.nginx_root).unwrap();
    (tmp, paths)
}

#[test]
fn existing_pair_is_reused_without_issuing() {
    let (_tmp, paths) = paths();
    for suffix in ["key", "crt", "fullchain.crt"] {
        std::fs::write(paths.nginx_root.join(format!("myapp.{suffix}")), "pem").unwrap();
    }

    let issued = issue_single("myapp", "myapp.203.0.113.10.example", &paths).unwrap();

    assert!(issued);
}

#[test]
fn without_acme_sh_there_is_no_certificate_and_no_self_signed() {
    let (_tmp, paths) = paths();

    let issued = issue_single("myapp", "myapp.203.0.113.10.example", &paths).unwrap();

    assert!(!issued);
    assert!(
        !paths.nginx_root.join("myapp.crt").exists(),
        "a fallback domain must never get a self-signed certificate"
    );
}

#[test]
fn invalid_app_name_is_refused() {
    let (_tmp, paths) = paths();

    assert!(issue_single("../x", "x.example", &paths).is_err());
}
