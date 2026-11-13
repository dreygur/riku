use std::collections::HashMap;

use super::{app_urls, live_at};
use crate::config::RikuPaths;
use crate::live_urls::{LiveUrl, UrlLabel};
use crate::router::routing_state::{with_routing_lock, Entry, RoutingMode};

fn enable_worker(paths: &RikuPaths, kind: &str) {
    std::fs::create_dir_all(&paths.workers_enabled).unwrap();
    let config = crate::supervisor::config::create_worker_config(
        "shop",
        kind,
        "run",
        1,
        HashMap::new(),
        "/srv/app",
        "/srv/log",
    );
    std::fs::write(
        paths.workers_enabled.join(format!("shop@{kind}.1.toml")),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();
}

/// `shop` with a web copy, last routed as `entry`, on a server whose
/// `server_ip` is a public documentation address.
fn routed_shop(tmp: &tempfile::TempDir, entry: Entry) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    enable_worker(&paths, "web");
    crate::config::settings::save_key(&paths, "routing", "server_ip", Some("203.0.113.10"))
        .unwrap();
    with_routing_lock(&paths, |state| {
        state.set_entry("shop", entry);
        Ok(())
    })
    .unwrap();
    paths
}

fn url(url: &str, label: UrlLabel) -> LiveUrl {
    LiveUrl {
        url: url.to_string(),
        label,
    }
}

fn shop_domain() -> HashMap<String, String> {
    HashMap::from([("RIKU_DOMAINS".to_string(), "shop.example".to_string())])
}

/// A Procfile with only `worker:` serves no HTTP, so the push summary must
/// not show a URL for it.
#[test]
fn a_worker_only_app_has_no_url() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    enable_worker(&paths, "worker");

    let lines = live_at("shop", &HashMap::new(), &paths, false);

    assert!(lines.is_empty(), "no URL lines expected: {lines:?}");
}

/// The router still answers on the port the last apply set up, so that is
/// where the app is live; the domain the settings now give only works after
/// a redeploy.
#[test]
fn an_app_routed_on_a_port_whose_settings_now_give_a_domain_reports_both() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = routed_shop(
        &tmp,
        Entry {
            mode: RoutingMode::Port,
            port: Some(8100),
            picked: true,
            tls: None,
            cert_for: None,
            domains: Vec::new(),
            fallback: false,
        },
    );

    let urls = app_urls("shop", &shop_domain(), &paths, false);

    assert_eq!(
        urls.urls,
        [url("http://203.0.113.10:8100", UrlLabel::Public)],
        "urls must be where the app is routed now"
    );
    assert_eq!(
        urls.pending,
        [url("http://shop.example", UrlLabel::Domain)],
        "pending must be what a redeploy would route"
    );
}

#[test]
fn an_app_whose_settings_match_its_route_has_no_pending_url() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = routed_shop(
        &tmp,
        Entry {
            mode: RoutingMode::Domain,
            port: None,
            picked: false,
            tls: None,
            cert_for: None,
            domains: vec!["shop.example".to_string()],
            fallback: false,
        },
    );

    let urls = app_urls("shop", &shop_domain(), &paths, false);

    assert_eq!(urls.urls, [url("http://shop.example", UrlLabel::Domain)]);
    assert!(urls.pending.is_empty(), "pending: {:?}", urls.pending);
}
