use super::*;
use crate::remote::types::ServerName;

fn server(admin: Option<&str>) -> Server {
    Server {
        name: ServerName::try_from("edge").unwrap(),
        address: SshAddress::try_from("riku@edge.example").unwrap(),
        admin: admin.map(|a| SshAddress::try_from(a).unwrap()),
    }
}

#[test]
fn remote_command_quotes_the_key() {
    let line = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample me@laptop; $(id) `id`";
    let command = remote_command(line).unwrap();
    let words = shlex::split(&command).unwrap();
    assert_eq!(words, ["sudo", "riku", "deploy-key", "add", line]);
}

#[test]
fn server_add_without_admin_prints_the_server_command() {
    let hint = manual_hint(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample",
        &server(None),
    );
    assert_eq!(
        hint,
        "Add the key on the server: sudo riku deploy-key add \
         'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample'\n\
         or rerun with --admin <your login>@edge.example"
    );
}
