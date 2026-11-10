use std::fs;

use super::super::Status;
use super::*;
use crate::own_cert::test_pems::{OLD_CERT, VALID_CERT};

const DAY: i64 = 86_400;

fn paths_with_cert(app: &str, cert: &str) -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.nginx_root).unwrap();
    fs::write(paths.nginx_root.join(format!("{app}.fullchain.crt")), cert).unwrap();
    fs::write(paths.nginx_root.join(format!("{app}.own-cert")), "").unwrap();
    (tmp, paths)
}

#[test]
fn warns_within_fourteen_days_and_fails_once_expired() {
    let (_tmp, paths) = paths_with_cert("myapp", VALID_CERT);
    let ends = crate::own_cert::info::read(VALID_CERT).unwrap().expires_at;

    let ok = own_certificates_at(&paths, ends - 30 * DAY);
    let soon = own_certificates_at(&paths, ends - 3 * DAY);
    let expired = own_certificates_at(&paths, ends + DAY);

    assert_eq!(ok[0].status, Status::Ok, "{}", ok[0].detail);
    assert_eq!(soon[0].status, Status::Warn, "{}", soon[0].detail);
    assert!(soon[0].detail.contains("myapp"), "got: {}", soon[0].detail);
    assert_eq!(expired[0].status, Status::Fail, "{}", expired[0].detail);
}

#[test]
fn a_marker_without_a_readable_certificate_fails() {
    let (_tmp, paths) = paths_with_cert("myapp", "not a certificate");

    let checks = own_certificates_at(&paths, 0);

    assert_eq!(checks[0].status, Status::Fail, "{}", checks[0].detail);
}

#[test]
fn apps_without_a_marker_are_not_checked() {
    let (_tmp, paths) = paths_with_cert("myapp", OLD_CERT);
    fs::remove_file(paths.nginx_root.join("myapp.own-cert")).unwrap();

    assert!(own_certificates_at(&paths, 0).is_empty());
}
