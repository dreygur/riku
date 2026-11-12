use super::*;
use tempfile::TempDir;

fn copy_one() -> CopyIndex {
    CopyIndex::new(1).unwrap()
}

fn persisted(paths: &RikuPaths) -> HashMap<String, String> {
    let mut settings = HashMap::new();
    crate::util::parse_settings(&paths.env_root.join("shop").join("ENV"), &mut settings).unwrap();
    settings
}

#[test]
fn switching_web_kind_drops_old_route_keys() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let base = HashMap::new();

    build_worker_env("shop", "wsgi", copy_one(), &base, &[], &paths).unwrap();
    build_worker_env("shop", "web", copy_one(), &base, &[], &paths).unwrap();
    let after_web = persisted(&paths);
    for key in ["RIKU_NGINX_WSGI", "UWSGI_SOCKET"] {
        assert!(
            !after_web.contains_key(key),
            "{key} left after wsgi to web: {after_web:?}"
        );
    }
    assert!(
        after_web.contains_key("RIKU_NGINX_PORTMAP"),
        "{after_web:?}"
    );

    build_worker_env("shop", "wsgi", copy_one(), &base, &[], &paths).unwrap();
    let after_wsgi = persisted(&paths);
    assert!(
        !after_wsgi.contains_key("RIKU_NGINX_PORTMAP"),
        "port map left after web to wsgi: {after_wsgi:?}"
    );
    assert!(after_wsgi.contains_key("RIKU_NGINX_WSGI"), "{after_wsgi:?}");
}

#[test]
fn a_port_set_in_the_app_env_is_used_and_no_port_is_persisted() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let base = HashMap::from([("PORT".to_string(), "3000".to_string())]);

    let env = build_worker_env("shop", "web", copy_one(), &base, &[], &paths).unwrap();

    assert_eq!(env["PORT"], "3000");
    assert!(
        !persisted(&paths).contains_key("RIKU_UPSTREAM_PORT"),
        "a persisted port lets the last copy win: {:?}",
        persisted(&paths)
    );
}

#[test]
fn a_copy_never_gets_a_port_another_copy_has() {
    let mut env = HashMap::new();
    let mut offered = [5001, 5001, 5002].into_iter();

    configure_web_env(Path::new("/s"), &mut env, &[5001], || {
        Ok(offered.next().expect("picker asked too often"))
    })
    .unwrap();

    assert_eq!(env["PORT"], "5002");
}

#[test]
fn gives_up_when_the_picker_only_offers_taken_ports() {
    let err = unused_port(&[5001], || Ok(5001)).unwrap_err();

    assert!(err.to_string().contains("distinct"), "{err}");
}

#[test]
fn custom_socket_is_where_worker_listens() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let base = HashMap::from([("RIKU_NGINX_SOCKET".to_string(), "/srv/s.sock".to_string())]);

    let env = build_worker_env("shop", "wsgi", copy_one(), &base, &[], &paths).unwrap();

    assert_eq!(env["UWSGI_SOCKET"], "/srv/s.sock");
    assert_eq!(env["SOCKET"], "unix:///srv/s.sock");
}

#[test]
fn each_wsgi_copy_listens_on_its_own_socket() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let second = CopyIndex::new(2).unwrap();

    let env = build_worker_env("shop", "wsgi", second, &HashMap::new(), &[], &paths).unwrap();

    let expected = paths.socket_root.join("shop@2.sock");
    assert_eq!(env["UWSGI_SOCKET"], expected.to_str().unwrap());
    assert!(
        paths.socket_root.is_dir(),
        "the socket directory was not created before uWSGI binds in it"
    );
}

#[test]
fn uwsgi_socket_is_group_only_and_stops_on_sigterm() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());

    let env = build_worker_env("shop", "php", copy_one(), &HashMap::new(), &[], &paths).unwrap();

    assert_eq!(
        env.get("UWSGI_CHMOD_SOCKET").map(String::as_str),
        Some("660")
    );
    assert_eq!(env.get("UWSGI_DIE_ON_TERM").map(String::as_str), Some("1"));
}

#[test]
fn rejects_a_pinned_port_that_is_not_a_port() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let base = HashMap::from([("PORT".to_string(), "$PORT".to_string())]);

    let err = build_worker_env("shop", "web", copy_one(), &base, &[], &paths).unwrap_err();

    assert!(err.to_string().contains("$PORT"), "{err}");
}
