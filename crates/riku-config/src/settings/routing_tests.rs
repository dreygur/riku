use std::net::Ipv4Addr;
use std::path::Path;

use super::*;
use crate::settings::parse;

fn file() -> &'static Path {
    Path::new("/srv/riku/settings.toml")
}

#[test]
fn routing_section_absent_gives_defaults_not_present() {
    let settings = parse("[router]\nname = \"nginx\"\n", file()).unwrap();

    assert_eq!(settings.routing, RoutingSettings::default());
    assert!(!settings.routing.present);
    assert_eq!(settings.routing.shared_port, 8080);
    assert_eq!(
        settings.routing.port_range,
        PortRange {
            first: 8100,
            last: 8199
        }
    );
}

#[test]
fn routing_section_values_are_read() {
    let raw = "[routing]\nfallback_domain = \"{app}.{ip}.example\"\nserver_ip = \"203.0.113.10\"\nshared_port = 9000\nport_range = \"9100-9110\"\n";

    let routing = parse(raw, file()).unwrap().routing;

    assert!(routing.present);
    assert_eq!(routing.server_ip, Some(Ipv4Addr::new(203, 0, 113, 10)));
    assert_eq!(routing.shared_port, 9000);
    assert_eq!(
        routing.port_range,
        PortRange {
            first: 9100,
            last: 9110
        }
    );
    assert!(routing.fallback_domain.is_some());
}

#[test]
fn fallback_pattern_without_app_is_refused() {
    let message = parse("[routing]\nfallback_domain = \"{ip}.example\"\n", file())
        .unwrap_err()
        .to_string();

    assert!(
        message.contains("routing.fallback_domain") && message.contains("{app}"),
        "error should name the key and the missing placeholder: {message}"
    );
}

#[test]
fn port_range_reversed_is_refused() {
    let message = parse("[routing]\nport_range = \"8199-8100\"\n", file())
        .unwrap_err()
        .to_string();

    assert!(message.contains("routing.port_range"), "{message}");
}

#[test]
fn port_range_below_1024_is_refused() {
    assert!(PortRange::try_from("80-90").is_err());
}

#[test]
fn expand_fills_app_and_ip() {
    let pattern = FallbackPattern::try_from("{app}.{ip}.example").unwrap();

    let domain = pattern
        .expand("myapp", Ipv4Addr::new(203, 0, 113, 10))
        .unwrap();

    assert_eq!(domain.as_str(), "myapp.203.0.113.10.example");
}

#[test]
fn expand_refuses_app_that_is_not_a_dns_label() {
    let pattern = FallbackPattern::try_from("{app}.{ip}.example").unwrap();

    let reason = pattern
        .expand("my_app", Ipv4Addr::new(203, 0, 113, 10))
        .unwrap_err();

    assert!(
        reason.contains("my_app"),
        "reason should name the app: {reason}"
    );
}

#[test]
fn ipv6_server_ip_is_refused_naming_the_key() {
    let message = parse("[routing]\nserver_ip = \"2001:db8::1\"\n", file())
        .unwrap_err()
        .to_string();

    assert!(
        message.contains("routing.server_ip") && message.contains("IPv4"),
        "{message}"
    );
}

#[test]
fn unknown_routing_key_names_the_key() {
    let message = parse("[routing]\nfallback = \"x\"\n", file())
        .unwrap_err()
        .to_string();

    assert!(message.contains("routing.fallback"), "{message}");
}

#[test]
fn routing_section_text_parses_back() {
    let with_ip = routing_section(Some(Ipv4Addr::new(203, 0, 113, 10)));
    let without_ip = routing_section(None);

    let parsed = parse(&with_ip, file()).unwrap().routing;
    assert_eq!(parsed.server_ip, Some(Ipv4Addr::new(203, 0, 113, 10)));
    assert!(parsed.fallback_domain.is_some());
    let parsed = parse(&without_ip, file()).unwrap().routing;
    assert!(parsed.present);
    assert_eq!(parsed.server_ip, None, "{without_ip}");
}
