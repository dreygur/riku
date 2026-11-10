use super::*;

const KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";

#[test]
fn deploy_key_add_refuses_non_root() {
    let err = add_deploy_key(false, KEY).unwrap_err();
    assert!(
        err.to_string().contains("sudo riku deploy-key add"),
        "a non root caller should be told to use sudo, got: {err}"
    );
}

#[test]
fn deploy_key_add_refuses_two_keys() {
    let err = add_deploy_key(true, &format!("{KEY}\n{KEY}")).unwrap_err();
    assert!(
        err.to_string().contains("exactly one key"),
        "two key lines should be refused, got: {err}"
    );
}

#[test]
fn key_with_options_lands_as_type_and_blob() {
    let line = format!("command=\"sh -c 'id'\",no-pty {KEY} me@laptop; $(id)");
    let key = single_key(&line).unwrap();
    assert_eq!(key.to_string(), KEY);
}
