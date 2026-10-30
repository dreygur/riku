use super::*;
use crate::remote::{ServerName, SshAddress};

fn server() -> Server {
    Server {
        name: ServerName::try_from("prod").unwrap(),
        address: SshAddress::try_from("riku@example.com").unwrap(),
        admin: None,
    }
}

#[test]
fn dashboard_is_the_only_server_command_that_resolves_a_server() {
    assert_eq!(routing("dashboard", None, true), Routing::Resolve);
    assert_eq!(routing("dashboard", None, false), Routing::Resolve);
    assert_eq!(routing("init", None, true), Routing::Refuse);
    assert_eq!(routing("init", None, false), Routing::Local);
}

#[test]
fn plugin_authoring_stays_local_while_other_plugins_commands_forward() {
    for authoring in ["keygen", "sign", "scaffold"] {
        assert_eq!(
            routing("plugins", Some(authoring), false),
            Routing::Local,
            "plugins {authoring} must not run on the server"
        );
    }
    assert_eq!(routing("plugins", Some("list"), false), Routing::Resolve);
    assert_eq!(routing("plugins", None, false), Routing::Resolve);
}

#[test]
fn target_line_names_the_app_taken_from_a_git_remote() {
    let prod = server();
    assert_eq!(
        target_line(&prod, Source::GitRemote, Some("blog")),
        "[*] prod (riku@example.com) app blog from git remote"
    );
    assert_eq!(
        target_line(&prod, Source::Default, None),
        "[*] prod (riku@example.com)"
    );
}
