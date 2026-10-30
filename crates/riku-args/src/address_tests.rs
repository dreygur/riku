use super::*;

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
    assert_eq!(address.as_str(), "riku@[::1]");
}

#[test]
fn rejects_server_name_with_bad_characters() {
    for bad in ["", "-lead", "Upper", "has space", "a/b", &"x".repeat(33)] {
        assert!(ServerName::try_from(bad).is_err(), "accepted {bad:?}");
    }
    assert!(ServerName::try_from("edge_2-a").is_ok());
}

#[test]
fn rejects_option_like_or_malformed_user_and_host() {
    for bad in [
        "riku@-x",
        "riku@a[b]c",
        "riku@host:22",
        "a:b@h",
        "-u@h",
        "riku@[::1]x",
        "riku@[g::1]",
        "riku@[]",
        "riku@[::1",
    ] {
        assert!(SshAddress::try_from(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn accepts_plain_hosts_and_users() {
    for good in [
        "riku@edge.example",
        "riku@192.168.1.20",
        "riku@[::1]",
        "deploy_user@host-1",
    ] {
        assert!(SshAddress::try_from(good).is_ok(), "rejected {good:?}");
    }
}
