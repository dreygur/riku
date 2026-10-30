use super::*;

fn parse(raw: &str) -> anyhow::Result<Domain> {
    Domain::try_from(raw)
}

#[test]
fn accepts_hosts_and_wildcard() {
    assert_eq!(
        parse("Shop.Example.com").unwrap().as_str(),
        "shop.example.com"
    );
    assert_eq!(parse("*.example.com").unwrap().as_str(), "*.example.com");
    assert_eq!(parse("app.localhost").unwrap().as_str(), "app.localhost");
}

#[test]
fn rejects_bad_labels() {
    let long_label = format!("{}.com", "a".repeat(64));
    let bad = [
        "a.com {",
        "a.com;b",
        "a b",
        "-a.com",
        "a..com",
        "a.*.com",
        "",
        "a.com.",
        &long_label,
    ];

    for raw in bad {
        let err = match parse(raw) {
            Ok(d) => panic!("{raw:?} was accepted as {d}"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains(raw), "error for {raw:?} should name it: {err}");
    }
}

#[test]
fn stops_at_first_bad_domain() {
    let good = parse_domains("a.com, b.com").unwrap();
    assert_eq!(good.len(), 2);

    let err = parse_domains("a.com, {x}").unwrap_err().to_string();
    assert!(
        err.contains("{x}"),
        "error should name the bad entry: {err}"
    );
}
