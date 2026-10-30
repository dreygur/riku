use super::*;
use crate::types::Protocol;
use serde_json::json;

fn binding(host_ip: HostIp, host: Option<u16>, container: u16, protocol: Protocol) -> PortBinding {
    PortBinding {
        host_ip,
        host_port: host,
        container_port: container,
        protocol,
    }
}

fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn short_and_long_forms_bind_the_given_interface() {
    let value = json!([
        "5432:5432",
        "9000",
        "53:53/udp",
        8080,
        {"target": 80, "published": "8000", "protocol": "udp"},
    ]);
    let ports = parse_ports(&value, HostIp::Loopback).unwrap();
    assert_eq!(
        ports,
        vec![
            binding(HostIp::Loopback, Some(5432), 5432, Protocol::Tcp),
            binding(HostIp::Loopback, None, 9000, Protocol::Tcp),
            binding(HostIp::Loopback, Some(53), 53, Protocol::Udp),
            binding(HostIp::Loopback, None, 8080, Protocol::Tcp),
            binding(HostIp::Loopback, Some(8000), 80, Protocol::Udp),
        ]
    );
    let public = parse_ports(&json!(["80:80"]), HostIp::All).unwrap();
    assert_eq!(public[0].host_ip, HostIp::All);
}

#[test]
fn explicit_host_ip_is_refused_in_both_forms() {
    for value in [
        json!(["0.0.0.0:80:80"]),
        json!(["127.0.0.1:80:80"]),
        json!(["[::]:80:80"]),
        json!([{"target": 80, "published": 80, "host_ip": "0.0.0.0"}]),
    ] {
        let err = parse_ports(&value, HostIp::Loopback)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("use the riku.public label instead of a host IP"),
            "{value}: {err}"
        );
    }
}

#[test]
fn ranges_modes_and_bad_numbers_are_refused() {
    for value in [
        json!(["8000-8010:8000-8010"]),
        json!(["0:80"]),
        json!(["99999:80"]),
        json!(["80:80/sctp"]),
        json!(["http"]),
        json!([{"target": 80, "mode": "host"}]),
        json!([{"published": 80}]),
    ] {
        assert!(
            parse_ports(&value, HostIp::Loopback).is_err(),
            "{value} was accepted"
        );
    }
}

#[test]
fn public_label_is_all_or_loopback_and_nothing_else() {
    assert_eq!(host_ip_for(&labels(&[])).unwrap(), HostIp::Loopback);
    assert_eq!(
        host_ip_for(&labels(&[("riku.public", "false")])).unwrap(),
        HostIp::Loopback
    );
    assert_eq!(
        host_ip_for(&labels(&[("riku.public", "true")])).unwrap(),
        HostIp::All
    );
    let err = host_ip_for(&labels(&[("riku.public", "yes")]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("riku.public") && err.contains("yes"), "{err}");
}
