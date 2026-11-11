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

    let check = ssh_access(home.path(), Reader::Owner);

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(check.detail.starts_with("2 riku"), "{}", check.detail);
}

/// Root reading the deploy user's keys must not follow a `.ssh` the deploy
/// user pointed at someone else's directory.
#[test]
fn root_refuses_a_symlinked_ssh_directory() {
    let target = home_with_keys("command=\"FINGERPRINT=SHA256:a riku\" ssh-ed25519 AAAA one\n");
    let home = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(target.path().join(".ssh"), home.path().join(".ssh")).unwrap();

    let check = ssh_access(home.path(), Reader::Root);

    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(check.detail.contains("not a directory"), "{}", check.detail);
}

#[test]
fn an_oversized_authorized_keys_is_not_read() {
    let line = "command=\"FINGERPRINT=SHA256:a riku\" ssh-ed25519 AAAA one\n";
    let home = home_with_keys(&line.repeat(MAX_LEN as usize / line.len() + 1));

    let check = ssh_access(home.path(), Reader::Owner);

    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(check.detail.contains("larger than"), "{}", check.detail);
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

    let check = ssh_access(home.path(), Reader::Root);

    assert_eq!(
        check.status,
        Status::Warn,
        "a symlink must not be followed: {}",
        check.detail
    );
}
