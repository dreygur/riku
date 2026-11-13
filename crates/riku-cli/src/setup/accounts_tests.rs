use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;

use super::*;

/// A PATH of one directory holding executables named `tools`.
fn path_with(tmp: &tempfile::TempDir, tools: &[&str]) -> OsString {
    for tool in tools {
        let file = tmp.path().join(tool);
        std::fs::write(&file, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    tmp.path().as_os_str().to_os_string()
}

const DEBIAN: &str = "PRETTY_NAME=\"Debian GNU/Linux 12\"\nID=debian\n";
const ALPINE: &str = "NAME=\"Alpine Linux\"\nID=alpine\n";

#[test]
fn shadow_utils_win_when_all_three_are_on_path() {
    let tmp = tempfile::tempdir().unwrap();
    let path = path_with(
        &tmp,
        &["useradd", "groupadd", "usermod", "adduser", "addgroup"],
    );

    assert_eq!(host_kind(&path, DEBIAN, false), Host::Tools(Tool::Shadow));
}

#[test]
fn busybox_is_used_without_shadow_utils() {
    let tmp = tempfile::tempdir().unwrap();
    let path = path_with(&tmp, &["adduser", "addgroup", "chpasswd", "useradd"]);

    assert_eq!(host_kind(&path, ALPINE, false), Host::Tools(Tool::BusyBox));
}

#[test]
fn nixos_is_told_by_its_marker_or_its_os_release_whatever_is_on_path() {
    let tmp = tempfile::tempdir().unwrap();
    let path = path_with(&tmp, &["useradd", "groupadd", "usermod"]);

    assert_eq!(host_kind(&path, "", true), Host::NixOs);
    assert_eq!(
        host_kind(&path, "NAME=NixOS\nID=nixos\n", false),
        Host::NixOs
    );
    assert_eq!(host_kind(&path, "ID=\"nixos\"\n", false), Host::NixOs);
    assert_eq!(
        host_kind(&path, "ID_LIKE=nixos\nID=other\n", false),
        Host::Tools(Tool::Shadow),
        "only ID names the distro"
    );
}

#[test]
fn no_tools_names_what_to_install() {
    let tmp = tempfile::tempdir().unwrap();
    let path = path_with(&tmp, &["useradd"]);

    assert_eq!(host_kind(&path, DEBIAN, false), Host::NoTools);
    let message = no_tools_message();
    assert!(
        message.contains("shadow") && message.contains("busybox"),
        "{message}"
    );
}

#[test]
fn shadow_commands_are_the_ones_init_always_ran() {
    assert_eq!(
        create_user_commands(Tool::Shadow, "riku", "/bin/bash"),
        [Cmd::new(
            "useradd",
            &[
                "--create-home",
                "--user-group",
                "--shell",
                "/bin/bash",
                "--password",
                "*",
                "--",
                "riku"
            ],
        )]
    );
    assert_eq!(
        create_group_command(Tool::Shadow, "riku-sockets"),
        Cmd::new("groupadd", &["--system", "riku-sockets"])
    );
    assert_eq!(
        add_to_group_command(Tool::Shadow, "riku", "riku-sockets"),
        Cmd::new("usermod", &["-aG", "riku-sockets", "riku"])
    );
}

/// BusyBox `adduser -D` leaves the password `!`, a locked account OpenSSH
/// refuses even for a key, so the `*` useradd sets comes from `chpasswd -e`.
#[test]
fn busybox_commands_map_every_useradd_flag() {
    let mut set_password = Cmd::new("chpasswd", &["-e"]);
    set_password.input = Some("riku:*\n".to_string());

    assert_eq!(
        create_user_commands(Tool::BusyBox, "riku", "/bin/bash"),
        [
            Cmd::new("addgroup", &["-S", "riku"]),
            Cmd::new(
                "adduser",
                &[
                    "-S",
                    "-D",
                    "-h",
                    "/home/riku",
                    "-s",
                    "/bin/bash",
                    "-G",
                    "riku",
                    "riku"
                ],
            ),
            set_password,
        ]
    );
    assert_eq!(
        create_group_command(Tool::BusyBox, "riku-sockets"),
        Cmd::new("addgroup", &["-S", "riku-sockets"])
    );
    assert_eq!(
        add_to_group_command(Tool::BusyBox, "riku", "riku-sockets"),
        Cmd::new("addgroup", &["riku", "riku-sockets"])
    );
}

#[test]
fn nixos_gets_the_configuration_lines_for_the_account_and_its_groups() {
    let block = nixos_account_lines("riku");

    for line in [
        "users.groups.riku = { };",
        "users.groups.riku-sockets = { };",
        "users.users.riku = {",
        "group = \"riku\";",
        "home = \"/home/riku\";",
        "extraGroups = [ \"riku-sockets\" ];",
        "hashedPassword = \"*\";",
    ] {
        assert!(block.contains(line), "missing {line:?} in:\n{block}");
    }
    assert_eq!(
        nixos_group_line("nginx", "riku-sockets"),
        "users.users.nginx.extraGroups = [ \"riku-sockets\" ];"
    );
}

#[test]
fn a_failed_command_is_printed_for_the_operator_to_run() {
    assert_eq!(
        Cmd::new("usermod", &["-aG", "docker", "riku"]).to_string(),
        "usermod -aG docker riku"
    );
}
