use super::*;
use crate::remote::git_remote::RemoteRef;
use crate::remote::{Server, ServerName, SshAddress};

fn server(name: &str, address: &str) -> Server {
    Server {
        name: ServerName::try_from(name).unwrap(),
        address: SshAddress::try_from(address).unwrap(),
        admin: None,
    }
}

fn list(with_default: bool) -> ServerList {
    let mut list = ServerList::default();
    list.add(server("solutech", "riku@solutech.local")).unwrap();
    list.add(server("prod", "riku@prod.example")).unwrap();
    if with_default {
        list.set_default(&ServerName::try_from("solutech").unwrap())
            .unwrap();
    }
    list
}

fn remote(address: &str, app: &str) -> RemoteRef {
    RemoteRef {
        address: address.to_string(),
        app: app.to_string(),
    }
}

fn sel(flag: Option<&str>) -> Selection<'_> {
    Selection {
        flag,
        local: false,
        via_ssh: false,
    }
}

fn remote_name(target: &Target) -> Option<&str> {
    match target {
        Target::Remote { server, .. } => Some(server.name.as_str()),
        Target::Local => None,
    }
}

#[test]
fn flag_beats_git_remote_and_default() {
    let remotes = [remote("riku@solutech.local", "shop")];
    let target = resolve(sel(Some("prod")), &list(true), &remotes).unwrap();
    match target {
        Target::Remote {
            server,
            source: Source::Flag,
            app_hint: None,
        } => assert_eq!(server.name.as_str(), "prod"),
        other => panic!("expected prod from the flag with no hint, got {other:?}"),
    }
}

#[test]
fn git_remote_beats_default_and_gives_app() {
    let remotes = [remote("riku@prod.example", "shop")];
    let target = resolve(sel(None), &list(true), &remotes).unwrap();
    match target {
        Target::Remote {
            server,
            source: Source::GitRemote,
            app_hint,
        } => {
            assert_eq!(server.name.as_str(), "prod");
            assert_eq!(app_hint.as_deref(), Some("shop"));
        }
        other => panic!("expected prod from the git remote, got {other:?}"),
    }
}

#[test]
fn two_matching_remotes_on_different_servers_is_an_error() {
    let remotes = [
        remote("riku@prod.example", "shop"),
        remote("riku@solutech.local", "shop"),
    ];
    let err = resolve(sel(None), &list(true), &remotes).unwrap_err();
    assert!(err.to_string().contains("-s"), "{err}");
}

#[test]
fn remotes_naming_different_apps_on_one_server_drop_the_hint() {
    let remotes = [
        remote("riku@prod.example", "shop"),
        remote("riku@prod.example", "shop-staging"),
    ];
    let target = resolve(sel(None), &list(true), &remotes).unwrap();
    match target {
        Target::Remote {
            server,
            source: Source::GitRemote,
            app_hint,
        } => {
            assert_eq!(server.name.as_str(), "prod");
            assert_eq!(app_hint, None, "ambiguous app should give no hint");
        }
        other => panic!("expected prod from the git remotes, got {other:?}"),
    }
}

#[test]
fn unregistered_remote_host_is_ignored() {
    let remotes = [remote("riku@10.0.0.9", "x")];
    let target = resolve(sel(None), &list(true), &remotes).unwrap();
    assert!(
        matches!(
            target,
            Target::Remote {
                source: Source::Default,
                ..
            }
        ),
        "{target:?}"
    );
    assert_eq!(remote_name(&target), Some("solutech"));
}

#[test]
fn ip_does_not_match_registered_hostname() {
    let remotes = [remote("riku@192.168.1.20", "x")];
    let target = resolve(sel(None), &list(false), &remotes).unwrap();
    assert!(matches!(target, Target::Local), "{target:?}");
}

#[test]
fn local_flag_forces_local() {
    let remotes = [remote("riku@prod.example", "shop")];
    let selection = Selection {
        flag: None,
        local: true,
        via_ssh: false,
    };
    let target = resolve(selection, &list(true), &remotes).unwrap();
    assert!(matches!(target, Target::Local), "{target:?}");
}

#[test]
fn ssh_invocation_never_forwards() {
    let selection = Selection {
        flag: Some("prod"),
        local: false,
        via_ssh: true,
    };
    let target = resolve(selection, &list(true), &[]).unwrap();
    assert!(matches!(target, Target::Local), "{target:?}");
}

#[test]
fn unknown_flag_server_is_an_error_listing_servers() {
    let err = resolve(sel(Some("nope")), &list(true), &[]).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("solutech") && text.contains("prod"), "{text}");
}
