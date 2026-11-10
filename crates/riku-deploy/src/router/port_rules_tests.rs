use std::collections::BTreeSet;

use super::*;
use crate::config::settings::{PortRange, RoutingSettings};
use crate::router::routing_state::{Entry, RoutingMode, RoutingState};

fn settings(first: u16, last: u16) -> RoutingSettings {
    RoutingSettings {
        port_range: PortRange { first, last },
        present: true,
        ..RoutingSettings::default()
    }
}

fn holding(app: &str, port: u16) -> RoutingState {
    let mut state = RoutingState::default();
    state.set_entry(
        app,
        Entry {
            mode: RoutingMode::Port,
            port: Some(port),
            picked: false,
            tls: None,
        },
    );
    state
}

fn none() -> BTreeSet<u16> {
    BTreeSet::new()
}

#[test]
fn port_held_by_other_app_is_refused_naming_it() {
    let state = holding("shop", 8181);

    let message = check_port("myapp", 8181, &state, &settings(8100, 8199), &none())
        .unwrap_err()
        .to_string();

    assert!(
        message.contains("shop") && message.contains("8181"),
        "{message}"
    );
}

#[test]
fn listening_port_is_refused() {
    let listening = BTreeSet::from([5432]);

    let message = check_port(
        "myapp",
        5432,
        &RoutingState::default(),
        &settings(8100, 8199),
        &listening,
    )
    .unwrap_err()
    .to_string();

    assert!(
        message.contains("5432") && message.contains("listening"),
        "{message}"
    );
}

#[test]
fn own_listening_port_is_allowed() {
    let state = holding("myapp", 8181);
    let listening = BTreeSet::from([8181]);

    assert!(check_port("myapp", 8181, &state, &settings(8100, 8199), &listening).is_ok());
}

#[test]
fn port_443_and_80_are_refused() {
    for port in [80, 443, 22] {
        let message = check_port(
            "myapp",
            port,
            &RoutingState::default(),
            &settings(8100, 8199),
            &none(),
        )
        .unwrap_err()
        .to_string();
        assert!(message.contains(&port.to_string()), "{message}");
    }
}

#[test]
fn caddy_admin_and_shared_port_are_refused() {
    for port in [2019, 8080] {
        assert!(
            check_port(
                "myapp",
                port,
                &RoutingState::default(),
                &settings(8100, 8199),
                &none()
            )
            .is_err(),
            "{port} must be refused"
        );
    }
}

#[test]
fn picked_port_is_lowest_free() {
    let state = holding("shop", 8100);
    let listening = BTreeSet::from([8101]);

    let port = pick_port("myapp", &state, &settings(8100, 8199), &listening).unwrap();

    assert_eq!(port, 8102);
}

#[test]
fn exhausted_port_range_is_refused() {
    let state = holding("shop", 8100);

    let message = pick_port("myapp", &state, &settings(8100, 8100), &none())
        .unwrap_err()
        .to_string();

    assert!(message.contains("8100-8100"), "{message}");
}
