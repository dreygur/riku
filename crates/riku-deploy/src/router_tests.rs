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

#[test]
fn worker_only_and_static_apps_keep_their_default_backend() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_workers(&tmp);
    enable_worker(&paths, "worker", 1, &[]);
    let socket = Backend::Unix(paths.nginx_root.join("shop.sock"));

    let worker_only = routed_backends(&Router::Nginx, "shop", &env(&[]), &paths, None).unwrap();
    let static_env = env(&[("RIKU_NGINX_STATIC", "true")]);
    let static_site = routed_backends(&Router::Nginx, "shop", &static_env, &paths, None).unwrap();

    assert_eq!(worker_only, vec![socket.clone()]);
    assert_eq!(static_site, vec![socket]);
}

#[test]
fn web_copies_replace_the_default_backend() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_workers(&tmp);
    enable_worker(&paths, "web", 1, &[("PORT", "5001")]);
    enable_worker(&paths, "web", 2, &[("PORT", "5002")]);
    let e = env(&[("RIKU_NGINX_PORTMAP", "true"), ("PORT", "5002")]);

    let backends = routed_backends(&Router::Nginx, "shop", &e, &paths, None).unwrap();

    assert_eq!(
        backends,
        vec![Backend::loopback(5001), Backend::loopback(5002)]
    );
}

#[test]
fn caddy_deploys_a_worker_only_app_to_the_port_from_its_env() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = paths_with_workers(&tmp);
    std::fs::write(&paths.router_file, "caddy").unwrap();
    std::fs::create_dir_all(&paths.caddy_root).unwrap();
    let ok = tmp.path().join("mock_caddy");
    std::fs::write(&ok, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&ok, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    paths.reload_programs.caddy = ok;
    enable_worker(&paths, "worker", 1, &[]);
    let e = env(&[("RIKU_DOMAINS", "shop.example.com"), ("PORT", "5001")]);

    configure("shop", tmp.path(), &e, &paths, None).unwrap();

    let site = std::fs::read_to_string(paths.caddy_root.join("shop.caddy")).unwrap();
    assert!(site.contains("reverse_proxy 127.0.0.1:5001"), "{site}");
}
