use super::*;

#[test]
fn forced_command_names_root_binary() {
    let home = tempfile::tempdir().unwrap();
    let key =
        PublicKey::try_from("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample me@laptop").unwrap();

    authorize_key(home.path(), "SHA256:ab", &key).unwrap();

    let written = authorized_keys_text(home.path()).unwrap();
    assert!(
        written.contains("command=\"FINGERPRINT=SHA256:ab NAME=default /usr/local/bin/riku\","),
        "forced command: {written}"
    );
    assert!(
        !written.contains(".local/bin"),
        "forced command names a path under the home: {written}"
    );
}
