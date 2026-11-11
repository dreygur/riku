use std::fs;

use super::*;
use crate::doctor::Status;

fn home_with_keys(keys: &str) -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(home.path().join(".ssh")).unwrap();
    fs::write(home.path().join(".ssh/authorized_keys"), keys).unwrap();
    home
}

#[test]
fn counts_the_riku_keys_of_the_home_it_is_given() {
    let home = home_with_keys(
        "command=\"FINGERPRINT=SHA256:a riku\" ssh-ed25519 AAAA one\n\
         ssh-ed25519 AAAA someone-else\n\
         command=\"FINGERPRINT=SHA256:b riku\" ssh-ed25519 AAAA two\n",
    );

    let check = ssh_access(home.path());

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(check.detail.starts_with("2 riku"), "{}", check.detail);
}

#[test]
fn refuses_an_authorized_keys_symlink() {
    let target = home_with_keys("command=\"FINGERPRINT=SHA256:a riku\" ssh-ed25519 AAAA one\n");
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(home.path().join(".ssh")).unwrap();
    std::os::unix::fs::symlink(
        target.path().join(".ssh/authorized_keys"),
        home.path().join(".ssh/authorized_keys"),
    )
    .unwrap();

    let check = ssh_access(home.path());

    assert_eq!(
        check.status,
        Status::Warn,
        "a symlink must not be followed: {}",
        check.detail
    );
}
