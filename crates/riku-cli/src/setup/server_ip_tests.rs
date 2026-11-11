use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::*;

#[test]
fn private_loopback_and_link_local_are_not_public_v4() {
    let refused = [
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
        IpAddr::V4(Ipv4Addr::new(172, 16, 4, 1)),
        IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
    ];
    for ip in refused {
        assert_eq!(public_v4(ip), None, "{ip} must not count as public");
    }
}

#[test]
fn public_address_is_kept() {
    let ip = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7));

    assert_eq!(public_v4(ip), Some(Ipv4Addr::new(198, 51, 100, 7)));
}

#[test]
fn routing_note_without_public_ip_says_how_to_set_it() {
    let note = routing_note(Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))), None).to_string();

    assert!(
        note.contains("10.0.0.5") && note.contains("server_ip"),
        "note should name the detected address and the key: {note}"
    );
}
