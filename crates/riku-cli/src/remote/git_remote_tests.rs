use super::*;

#[test]
fn parses_scp_style_and_home_path_remotes() {
    let out = "riku\triku@host:myapp (push)\nold\triku@host:~/x/shop.git (fetch)\ngh\thttps://github.com/a/b.git (fetch)\n";
    let apps: Vec<(String, String)> = parse_remotes(out)
        .into_iter()
        .map(|r| (r.address, r.app))
        .collect();
    assert_eq!(
        apps,
        [
            ("riku@host".to_string(), "myapp".to_string()),
            ("riku@host".to_string(), "shop".to_string())
        ]
    );
}

#[test]
fn bracketed_ipv6_host_keeps_its_colons() {
    let remotes = parse_remotes("r\triku@[fe80::1]:myapp (push)\n");
    assert_eq!(remotes.len(), 1, "{remotes:?}");
    assert_eq!(remotes[0].address, "riku@[fe80::1]");
    assert_eq!(remotes[0].app, "myapp");
}
