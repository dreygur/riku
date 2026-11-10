use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::own_cert::bundle::validate;
use crate::own_cert::test_pems::{VALID_CERT, VALID_KEY};

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.nginx_root).unwrap();
    (tmp, paths)
}

fn file(paths: &RikuPaths, name: &str) -> std::path::PathBuf {
    paths.nginx_root.join(name)
}

#[test]
fn install_writes_the_pair_private_and_the_marker_last() {
    let (_tmp, paths) = paths();
    let bundle = validate(&format!("{VALID_CERT}{VALID_CERT}{VALID_KEY}")).unwrap();

    install(&paths, "myapp", &bundle).unwrap();

    let mode = |name: &str| {
        fs::metadata(file(&paths, name))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode("myapp.key"), 0o600);
    let chain = fs::read_to_string(file(&paths, "myapp.fullchain.crt")).unwrap();
    let leaf = fs::read_to_string(file(&paths, "myapp.crt")).unwrap();
    assert_eq!(chain.matches("BEGIN CERTIFICATE").count(), 2);
    assert_eq!(leaf.matches("BEGIN CERTIFICATE").count(), 1);
    assert_eq!(source(&paths, "myapp"), Source::Own);
}

#[test]
fn source_tells_the_four_cases_apart() {
    let (_tmp, paths) = paths();
    assert_eq!(source(&paths, "myapp"), Source::None);

    fs::write(file(&paths, "myapp.crt"), "x").unwrap();
    assert_eq!(source(&paths, "myapp"), Source::SelfSigned);

    fs::write(file(&paths, "myapp.fullchain.crt"), "x").unwrap();
    assert_eq!(source(&paths, "myapp"), Source::LetsEncrypt);

    fs::write(file(&paths, "myapp.own-cert"), "").unwrap();
    assert_eq!(source(&paths, "myapp"), Source::Own);
}

#[test]
fn remove_deletes_the_pair_and_marker_of_one_app_only() {
    let (_tmp, paths) = paths();
    for name in [
        "myapp.key",
        "myapp.crt",
        "myapp.fullchain.crt",
        "myapp.own-cert",
        "other.key",
    ] {
        fs::write(file(&paths, name), "x").unwrap();
    }

    remove(&paths, "myapp").unwrap();

    assert!(!file(&paths, "myapp.own-cert").exists());
    assert!(!file(&paths, "myapp.key").exists());
    assert!(file(&paths, "other.key").exists());
}

#[test]
fn leaf_comes_from_the_chain_when_there_is_one() {
    let (_tmp, paths) = paths();
    assert_eq!(leaf_pem(&paths, "myapp"), None);

    fs::write(file(&paths, "myapp.crt"), VALID_CERT).unwrap();
    assert_eq!(leaf_pem(&paths, "myapp").as_deref(), Some(VALID_CERT));
}
