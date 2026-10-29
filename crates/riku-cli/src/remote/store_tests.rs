use super::*;
use crate::remote::{Server, ServerName, SshAddress};
use std::os::unix::fs::PermissionsExt;

#[test]
fn round_trips_through_toml_with_mode_600() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("riku").join("servers.toml");
    let mut list = ServerList::default();
    list.add(Server {
        name: ServerName::try_from("solutech").unwrap(),
        address: SshAddress::try_from("riku@solutech.local").unwrap(),
        admin: Some(SshAddress::try_from("dreygur@solutech.local").unwrap()),
    })
    .unwrap();
    list.set_default(&ServerName::try_from("solutech").unwrap())
        .unwrap();

    save(&path, &list).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "servers file mode");
    let dir_mode = std::fs::metadata(path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700, "config dir mode");
    assert_eq!(load(&path).unwrap(), list);
}

#[test]
fn missing_file_loads_empty() {
    let dir = tempfile::tempdir().unwrap();
    let list = load(&dir.path().join("none.toml")).unwrap();
    assert_eq!(list.iter().count(), 0);
}

#[test]
fn bad_entry_names_file_and_server() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.toml");
    std::fs::write(
        &path,
        "[[server]]\nname = \"broken\"\naddress = \"-oProxyCommand=x@h\"\n",
    )
    .unwrap();
    let message = format!("{:#}", load(&path).unwrap_err());
    assert!(message.contains("servers.toml"), "no file in: {message}");
    assert!(message.contains("broken"), "no server name in: {message}");
}

#[test]
fn servers_path_prefers_xdg_config_home() {
    assert_eq!(
        servers_path_from(Some("/x/cfg"), "/home/me"),
        PathBuf::from("/x/cfg/riku/servers.toml")
    );
    assert_eq!(
        servers_path_from(None, "/home/me"),
        PathBuf::from("/home/me/.config/riku/servers.toml")
    );
}

#[test]
fn relative_xdg_config_home_is_ignored() {
    assert_eq!(
        servers_path_from(Some("relative/cfg"), "/home/me"),
        PathBuf::from("/home/me/.config/riku/servers.toml")
    );
}

fn load_text(text: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.toml");
    std::fs::write(&path, text).unwrap();
    format!("{:#}", load(&path).unwrap_err())
}

#[test]
fn default_naming_missing_server_is_an_error() {
    let message =
        load_text("default = \"ghost\"\n[[server]]\nname = \"a\"\naddress = \"riku@a.local\"\n");
    assert!(
        message.contains("servers.toml") && message.contains("ghost"),
        "{message}"
    );
}

#[test]
fn duplicate_server_in_file_is_an_error() {
    let message = load_text(
        "[[server]]\nname = \"a\"\naddress = \"riku@a.local\"\n[[server]]\nname = \"a\"\naddress = \"riku@b.local\"\n",
    );
    assert!(
        message.contains("servers.toml") && message.contains("'a'"),
        "{message}"
    );
}
