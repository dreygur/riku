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

/// Init runs this on every run, so an existing settings.toml without the
/// section gets it, and a second run leaves the file alone.
#[test]
fn routing_section_is_added_to_an_existing_file_once() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(&paths.settings_file, "[router]\nname = \"caddy\"\n").unwrap();

    assert!(ensure_routing_section(&paths).unwrap().is_some());
    let after_first = std::fs::read_to_string(&paths.settings_file).unwrap();
    assert!(ensure_routing_section(&paths).unwrap().is_none());

    let loaded = crate::config::settings::load(&paths).unwrap();
    assert!(loaded.routing.present, "{after_first}");
    assert_eq!(loaded.router.as_deref(), Some("caddy"));
    assert_eq!(
        std::fs::read_to_string(&paths.settings_file).unwrap(),
        after_first
    );
}

#[test]
fn routing_note_without_public_ip_says_how_to_set_it() {
    let note = routing_note(Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))), None).to_string();

    assert!(
        note.contains("10.0.0.5") && note.contains("server_ip"),
        "note should name the detected address and the key: {note}"
    );
}
