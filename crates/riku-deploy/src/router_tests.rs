use super::*;

pub(super) fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn route_plan_carries_domains_policy_and_the_given_backends() {
    let e = env(&[
        ("RIKU_DOMAINS", "shop.example.com"),
        ("RIKU_HTTPS_ONLY", "false"),
    ]);

    let plan = route_plan(&e, &[Backend::loopback(3000)]).unwrap();

    assert_eq!(plan.domains.len(), 1);
    assert_eq!(plan.https, HttpsPolicy::AlsoPlainHttp);
    assert_eq!(plan.backends, vec![Backend::loopback(3000)]);
}

#[test]
fn route_plan_rejects_bad_domain() {
    let err = route_plan(&env(&[("RIKU_DOMAINS", "a.com {")]), &[]).unwrap_err();

    assert!(err.to_string().contains("a.com {"), "{err}");
}

#[test]
fn caddy_proxies_to_loopback_and_others_keep_the_copy_address() {
    let host = "10.0.0.5".parse().unwrap();
    let copies = vec![
        Backend::Tcp { host, port: 5001 },
        Backend::Unix("/srv/a.sock".into()),
    ];
    let plugin = Router::Plugin("edge-router".to_string());

    assert_eq!(
        backends_for(&Router::Caddy, copies.clone()),
        vec![Backend::loopback(5001), Backend::Unix("/srv/a.sock".into())]
    );
    assert_eq!(backends_for(&plugin, copies.clone()), copies);
    assert_eq!(backends_for(&Router::Nginx, copies.clone()), copies);
}

fn paths_with_workers(tmp: &tempfile::TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.workers_enabled).unwrap();
    paths
}

fn enable_worker(paths: &RikuPaths, kind: &str, copy: u32, pairs: &[(&str, &str)]) {
    let config = crate::supervisor::config::create_worker_config(
        "shop",
        kind,
        "run",
        copy,
        env(pairs),
        "/srv/app",
        "/srv/log",
    );
    let file = paths
        .workers_enabled
        .join(format!("shop@{kind}.{copy}.toml"));
    std::fs::write(file, toml::to_string(&config).unwrap()).unwrap();
}

/// A static site has no web copy but nginx serves its files, so it keeps a
/// route to its default backend. A worker only app serves nothing and gets
/// no route at all.
#[test]
fn only_a_static_app_without_web_copies_is_routed() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_workers(&tmp);
    enable_worker(&paths, "worker", 1, &[]);
    let socket = Backend::Unix(paths.socket_root.join("shop.sock"));
    let static_env = env(&[("RIKU_NGINX_STATIC", "true")]);

    let static_site = routed_backends(&Router::Nginx, "shop", &static_env, &paths, None).unwrap();

    assert_eq!(static_site, vec![socket]);
    assert!(has_route("shop", &static_env, &paths).unwrap());
    assert!(
        !has_route("shop", &env(&[]), &paths).unwrap(),
        "a worker only app must get no route"
    );
}

#[test]
fn web_copies_replace_the_default_backend() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_workers(&tmp);
    enable_worker(&paths, "web", 1, &[("PORT", "5001")]);
    enable_worker(&paths, "web", 2, &[("PORT", "5002")]);
    let e = env(&[("RIKU_NGINX_PORTMAP", "true"), ("PORT", "5002")]);

    let backends = routed_backends(&Router::Nginx, "shop", &e, &paths, None).unwrap();

    assert!(
        has_route("shop", &e, &paths).unwrap(),
        "a web app is routed"
    );
    assert_eq!(
        backends,
        vec![Backend::loopback(5001), Backend::loopback(5002)]
    );
}

/// The check runs before any copy starts, so a Caddy app that sets
/// `routing.static_paths` fails there, not after its workers are up.
#[test]
fn caddy_check_refuses_static_paths_before_a_copy_starts() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_workers(&tmp);
    crate::config::settings::save_key(&paths, "router", "name", Some("caddy")).unwrap();
    let e = env(&[
        ("RIKU_DOMAINS", "shop.example.com"),
        ("RIKU_NGINX_STATIC_PATHS", "/assets/:public"),
    ]);

    let err = check("shop", &e, &paths).unwrap_err();

    assert!(
        format!("{err:#}").contains("routing.static_paths is nginx only"),
        "{err:#}"
    );
}

/// A worker only app that was never routed has nothing to take off the
/// router, so its deploy must not need the router helper. One with a route
/// recorded still goes through the helper, and fails when it is missing.
#[test]
fn a_worker_only_app_needs_the_helper_only_to_drop_a_recorded_route() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = paths_with_workers(&tmp);
    paths.router_socket = tmp.path().join("no-helper.sock");
    enable_worker(&paths, "worker", 1, &[]);

    configure("shop", tmp.path(), &env(&[]), &paths, None).unwrap();

    routing_state::with_routing_lock(&paths, |state| {
        state.set_entry(
            "shop",
            routing_state::Entry {
                mode: RoutingMode::Domain,
                port: None,
                picked: false,
                tls: None,
                cert_for: None,
                domains: Vec::new(),
                fallback: false,
            },
        );
        Ok(())
    })
    .unwrap();
    let err = configure("shop", tmp.path(), &env(&[]), &paths, None).unwrap_err();
    assert!(format!("{err:#}").contains("router helper"), "{err:#}");
}

/// Path mode never builds a per app route, so it must refuse the key itself,
/// before the routing state or the helper sees the app.
#[test]
fn path_mode_app_that_names_its_socket_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_workers(&tmp);
    let e = env(&[
        ("RIKU_NGINX_STATIC", "true"),
        ("RIKU_ROUTING_MODE", "path"),
        ("RIKU_NGINX_SOCKET", "/srv/s.sock"),
    ]);
    let backends = [Backend::Unix("/srv/s.sock".into())];

    let configured = configure("shop", tmp.path(), &e, &paths, None).unwrap_err();
    let applied = apply("shop", tmp.path(), &e, &paths, &backends).unwrap_err();

    for err in [configured, applied] {
        assert!(
            format!("{err:#}").contains("RIKU_NGINX_SOCKET cannot be set by an app"),
            "{err:#}"
        );
    }
    assert!(
        previous_entry("shop", &paths).is_none(),
        "a refused app must not be recorded in path mode"
    );
}

#[test]
fn caddy_publishes_no_site_for_a_worker_only_app() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = paths_with_workers(&tmp);
    crate::config::settings::save_key(&paths, "router", "name", Some("caddy")).unwrap();
    std::fs::create_dir_all(&paths.caddy_root).unwrap();
    let ok = tmp.path().join("mock_caddy");
    std::fs::write(&ok, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&ok, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    paths.reload_programs.caddy = ok;
    riku_route::serve_caddy_for_tests(&mut paths);
    enable_worker(&paths, "worker", 1, &[]);
    let e = env(&[("RIKU_DOMAINS", "shop.example.com"), ("PORT", "5001")]);

    configure("shop", tmp.path(), &e, &paths, None).unwrap();

    assert!(
        !paths.caddy_root.join("shop.caddy").exists(),
        "nothing of a worker only app answers HTTP, so it gets no site"
    );
}
