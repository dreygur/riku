use std::fs;

use nix::unistd::Uid;

use super::*;

const ME: u32 = 1000;

#[test]
fn a_system_unit_marks_a_system_install_even_for_the_deploy_user() {
    let tmp = tempfile::tempdir().unwrap();
    let unit = tmp.path().join("riku.service");
    fs::write(&unit, "[Unit]\n").unwrap();

    let found = system_install(&unit, Some(Uid::from_raw(ME)), Uid::from_raw(ME));

    assert_eq!(
        found,
        Some(SystemInstall::Unit(unit)),
        "a system riku.service must count as a system install"
    );
}

#[test]
fn another_account_as_deploy_user_marks_a_system_install() {
    let tmp = tempfile::tempdir().unwrap();
    let unit = tmp.path().join("riku.service");

    let found = system_install(&unit, Some(Uid::from_raw(999)), Uid::from_raw(ME));

    assert!(
        found.is_some(),
        "a deploy account other than the caller means root installed riku"
    );
}

#[test]
fn single_user_mode_is_not_a_system_install() {
    let tmp = tempfile::tempdir().unwrap();
    let unit = tmp.path().join("riku.service");

    let no_account = system_install(&unit, None, Uid::from_raw(ME));
    let caller_is_account = system_install(&unit, Some(Uid::from_raw(ME)), Uid::from_raw(ME));

    assert_eq!(no_account, None, "no unit and no deploy account");
    assert_eq!(
        caller_is_account, None,
        "no unit, and the caller is the deploy account"
    );
}

#[test]
fn unit_refusal_names_the_unit_and_both_ways_out() {
    let unit = std::path::PathBuf::from("/etc/systemd/system/riku.service");

    let message = format!("{:#}", refusal(&SystemInstall::Unit(unit)));

    for part in [
        "/etc/systemd/system/riku.service exists",
        "sudo riku init",
        "remove /etc/systemd/system/riku.service",
    ] {
        assert!(message.contains(part), "missing '{part}' in: {message}");
    }
    assert!(
        !message.contains("RIKU_USER"),
        "the account is not the blocker here: {message}"
    );
}

#[test]
fn account_refusal_names_the_account_and_both_ways_out() {
    let message = format!(
        "{:#}",
        refusal(&SystemInstall::DeployAccount("riku".to_string()))
    );

    for part in [
        "the deploy account riku exists",
        "sudo riku init",
        "set RIKU_USER to another deploy user name",
    ] {
        assert!(message.contains(part), "missing '{part}' in: {message}");
    }
    assert!(
        !message.contains("remove /etc"),
        "the unit is not the blocker here: {message}"
    );
}
