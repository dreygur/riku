use super::*;

fn server(name: &str, address: &str) -> Server {
    Server {
        name: ServerName::try_from(name).unwrap(),
        address: SshAddress::try_from(address).unwrap(),
        admin: None,
    }
}

#[test]
fn rejects_address_starting_with_dash() {
    let result = SshAddress::try_from("-oProxyCommand=x@h");
    assert!(result.is_err(), "leading dash must be rejected: {result:?}");
}

#[test]
fn rejects_address_with_shell_metacharacters() {
    for bad in [
        "riku@host;rm",
        "riku@ho st",
        "riku@$(id)",
        "riku@a@b",
        "@host",
        "riku@",
    ] {
        assert!(SshAddress::try_from(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn accepts_ipv6_host() {
    let address = SshAddress::try_from("riku@[::1]").expect("ipv6 literal should parse");
    assert_eq!(address.user(), "riku");
    assert_eq!(address.host(), "[::1]");
    assert_eq!(address.as_str(), "riku@[::1]");
}

#[test]
fn rejects_server_name_with_bad_characters() {
    for bad in ["", "-lead", "Upper", "has space", "a/b", &"x".repeat(33)] {
        assert!(ServerName::try_from(bad).is_err(), "accepted {bad:?}");
    }
    assert!(ServerName::try_from("solutech_2-a").is_ok());
}

#[test]
fn removing_default_clears_it() {
    let mut list = ServerList::default();
    list.add(server("a", "riku@a.local")).unwrap();
    let name = ServerName::try_from("a").unwrap();
    list.set_default(&name).unwrap();
    assert!(list.default_server().is_some());
    list.remove(&name).unwrap();
    assert!(list.default_server().is_none(), "default survived removal");
    assert!(list.get(&name).is_none());
}

#[test]
fn duplicate_name_is_rejected() {
    let mut list = ServerList::default();
    list.add(server("a", "riku@a.local")).unwrap();
    assert!(list.add(server("a", "riku@b.local")).is_err());
    assert_eq!(list.iter().count(), 1);
}
