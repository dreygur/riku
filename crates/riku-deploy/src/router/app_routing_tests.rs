use std::collections::{BTreeSet, HashMap};
use std::net::Ipv4Addr;

use super::*;
use crate::config::settings::{FallbackPattern, RoutingSettings};
use crate::config::RikuPaths;
use crate::router::routing_state::with_routing_lock;

fn routing(fallback: bool) -> RoutingSettings {
    RoutingSettings {
        fallback_domain: fallback.then(|| FallbackPattern::try_from("{app}.{ip}.example").unwrap()),
        server_ip: Some(Ipv4Addr::new(203, 0, 113, 10)),
        present: true,
        ..RoutingSettings::default()
    }
}

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn resolve_fresh(env: &HashMap<String, String>, settings: &RoutingSettings) -> Result<AppRouting> {
    let mut state = RoutingState::default();
    resolve("myapp", env, settings, &mut state, &BTreeSet::new(), false)
}

#[test]
fn no_domain_and_fallback_resolves_to_domain_mode() {
    let routed = resolve_fresh(&env(&[]), &routing(true)).unwrap();

    assert_eq!(routed.mode, RoutingMode::Domain);
    assert_eq!(
        routed.domains,
        vec!["myapp.203.0.113.10.example".to_string()]
    );
    assert!(routed.fallback);
    assert_eq!(routed.port, None);
}

#[test]
fn a_fallback_name_on_a_private_address_is_marked_http_only() {
    let private_ip = Ipv4Addr::new(10, 0, 0, 5);
    let private = RoutingSettings {
        server_ip: Some(private_ip),
        ..routing(true)
    };
    let without_ip = RoutingSettings {
        fallback_domain: Some(FallbackPattern::try_from("{app}.apps.example").unwrap()),
        ..private.clone()
    };

    let on_private = resolve_fresh(&env(&[]), &private).unwrap();
    let on_public = resolve_fresh(&env(&[]), &routing(true)).unwrap();
    let dns_name = resolve_fresh(&env(&[]), &without_ip).unwrap();

    assert_eq!(on_private.domains, ["myapp.10.0.0.5.example"]);
    assert_eq!(on_private.fallback_private_ip, Some(private_ip));
    assert_eq!(on_public.fallback_private_ip, None);
    assert_eq!(
        dns_name.fallback_private_ip, None,
        "a name without {{ip}} resolves through DNS, so its address is not known to be private"
    );
}

#[test]
fn no_domain_no_fallback_resolves_to_port_mode() {
    let routed = resolve_fresh(&env(&[]), &routing(false)).unwrap();

    assert_eq!(routed.mode, RoutingMode::Port);
    assert_eq!(routed.port, Some(8100));
    assert!(routed.domains.is_empty());
}

#[test]
fn domainless_uwsgi_or_static_app_stays_out_of_port_mode() {
    for key in ["RIKU_NGINX_WSGI", "RIKU_NGINX_STATIC"] {
        let routed = resolve_fresh(&env(&[(key, "true")]), &routing(false)).unwrap();

        assert_eq!(routed.mode, RoutingMode::Domain, "{key}");
        assert_eq!(routed.port, None, "{key}");
    }
}

#[test]
fn own_domains_win_over_fallback() {
    let routed = resolve_fresh(
        &env(&[("RIKU_DOMAINS", "shop.example,www.shop.example")]),
        &routing(true),
    )
    .unwrap();

    assert_eq!(routed.domains, vec!["shop.example", "www.shop.example"]);
    assert!(!routed.fallback);
}

#[test]
fn fallback_skipped_for_app_name_not_a_dns_label() {
    let mut state = RoutingState::default();

    let routed = resolve(
        "my_app",
        &env(&[]),
        &routing(true),
        &mut state,
        &BTreeSet::new(),
        false,
    )
    .unwrap();

    assert_eq!(routed.mode, RoutingMode::Port);
    assert!(
        routed
            .fallback_skipped
            .as_deref()
            .is_some_and(|why| why.contains("my_app")),
        "{:?}",
        routed.fallback_skipped
    );
}

#[test]
fn no_routing_section_uses_the_defaults() {
    let routed = resolve_fresh(&env(&[]), &RoutingSettings::default()).unwrap();

    assert_eq!(routed.mode, RoutingMode::Port, "{routed:?}");
    assert!(routed.domains.is_empty(), "{routed:?}");
    assert!(
        routed
            .port
            .is_some_and(|port| (8100..=8199).contains(&port)),
        "port from the default range: {routed:?}"
    );
}

#[test]
fn domain_and_port_answer_on_both() {
    let routed = resolve_fresh(
        &env(&[
            ("RIKU_DOMAINS", "shop.example"),
            ("RIKU_ROUTING_PORT", "8181"),
        ]),
        &routing(true),
    )
    .unwrap();

    assert_eq!(routed.mode, RoutingMode::Domain);
    assert_eq!(routed.port, Some(8181));
}

#[test]
fn path_mode_with_domains_or_port_is_refused() {
    for extra in [
        ("RIKU_DOMAINS", "shop.example"),
        ("RIKU_ROUTING_PORT", "8181"),
    ] {
        let result = resolve_fresh(
            &env(&[("RIKU_ROUTING_MODE", "path"), extra]),
            &routing(true),
        );
        assert!(result.is_err(), "path mode with {extra:?} must be refused");
    }
}

#[test]
fn port_mode_under_plugin_router_is_refused() {
    let mut state = RoutingState::default();

    let result = resolve(
        "myapp",
        &env(&[("RIKU_ROUTING_MODE", "port")]),
        &routing(true),
        &mut state,
        &BTreeSet::new(),
        true,
    );

    assert!(result.is_err());
}

#[test]
fn picked_port_is_reused_next_deploy() {
    let mut state = RoutingState::default();
    let settings = routing(false);
    resolve(
        "other",
        &env(&[]),
        &settings,
        &mut state,
        &BTreeSet::new(),
        false,
    )
    .unwrap();
    let first = resolve(
        "myapp",
        &env(&[]),
        &settings,
        &mut state,
        &BTreeSet::new(),
        false,
    )
    .unwrap();

    let listening = BTreeSet::from([first.port.unwrap()]);
    let second = resolve("myapp", &env(&[]), &settings, &mut state, &listening, false).unwrap();

    assert_eq!(first.port, Some(8101));
    assert_eq!(second.port, first.port);
}

#[test]
fn gaining_a_domain_frees_the_picked_port() {
    let mut state = RoutingState::default();
    let settings = routing(false);
    resolve(
        "myapp",
        &env(&[]),
        &settings,
        &mut state,
        &BTreeSet::new(),
        false,
    )
    .unwrap();

    let routed = resolve(
        "myapp",
        &env(&[("RIKU_DOMAINS", "shop.example")]),
        &settings,
        &mut state,
        &BTreeSet::new(),
        false,
    )
    .unwrap();

    assert_eq!(routed.port, None);
    assert_eq!(state.entry("myapp").and_then(|e| e.port), None);
}

#[test]
fn concurrent_picks_get_different_ports() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    let settings = routing(false);

    let ports: Vec<u16> = std::thread::scope(|scope| {
        let handles: Vec<_> = ["one", "two", "three", "four"]
            .into_iter()
            .map(|app| {
                let (paths, settings) = (&paths, &settings);
                scope.spawn(move || {
                    with_routing_lock(paths, |state| {
                        resolve(
                            app,
                            &HashMap::new(),
                            settings,
                            state,
                            &BTreeSet::new(),
                            false,
                        )
                    })
                    .unwrap()
                    .port
                    .unwrap()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let distinct: BTreeSet<u16> = ports.iter().copied().collect();
    assert_eq!(distinct.len(), ports.len(), "ports: {ports:?}");
}
