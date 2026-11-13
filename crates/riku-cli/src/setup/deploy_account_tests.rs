use super::*;

#[test]
fn accepts_a_plain_login_name() {
    let name = AccountName::try_from("riku".to_string()).unwrap();
    assert_eq!(name.as_str(), "riku");
}

#[test]
fn rejects_names_useradd_would_misread_or_refuse() {
    for bad in [
        "",
        "-r",
        "Riku",
        "1riku",
        "ri ku",
        "../riku",
        &"r".repeat(33),
    ] {
        assert!(
            AccountName::try_from(bad.to_string()).is_err(),
            "{bad:?} should be rejected"
        );
    }
}

/// An account an older init made with /bin/bash moves to /bin/sh through
/// usermod; hosts without usermod, and NixOS, where configuration.nix owns
/// the account, are told instead, and nothing runs. Any other shell stays.
#[test]
fn an_old_login_shell_is_changed_only_where_riku_manages_accounts() {
    use super::super::accounts::{Cmd, Host, Tool};

    assert_eq!(
        shell_action("/bin/sh", Host::Tools(Tool::Shadow), "riku"),
        None
    );
    assert_eq!(
        shell_action("/usr/bin/zsh", Host::Tools(Tool::Shadow), "riku"),
        None,
        "a shell riku never set was someone's choice"
    );
    assert_eq!(
        shell_action("/bin/bash", Host::Tools(Tool::Shadow), "riku"),
        Some(ShellAction::Run(Cmd::new(
            "usermod",
            &["--shell", "/bin/sh", "--", "riku"]
        )))
    );
    for host in [Host::Tools(Tool::BusyBox), Host::NixOs, Host::NoTools] {
        match shell_action("/bin/bash", host, "riku") {
            Some(ShellAction::Explain(text)) => {
                assert!(text.contains("/bin/sh"), "{host:?}: {text}");
                if host == Host::NixOs {
                    assert!(text.contains("configuration.nix"), "{text}");
                }
            }
            other => panic!("{host:?} must explain, not run: {other:?}"),
        }
    }
}
