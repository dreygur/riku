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

    build_worker_env("shop", "wsgi", copy_one(), &base, &paths).unwrap();
    build_worker_env("shop", "web", copy_one(), &base, &paths).unwrap();
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

    build_worker_env("shop", "wsgi", copy_one(), &base, &paths).unwrap();
    let after_wsgi = persisted(&paths);
    for key in ["RIKU_NGINX_PORTMAP", "RIKU_NGINX_EXTERNAL_PORT"] {
        assert!(
            !after_wsgi.contains_key(key),
            "{key} left after web to wsgi: {after_wsgi:?}"
        );
    }
    assert!(after_wsgi.contains_key("RIKU_NGINX_WSGI"), "{after_wsgi:?}");
}

#[test]
fn a_port_set_in_the_app_env_is_used_and_no_port_is_persisted() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let base = HashMap::from([("PORT".to_string(), "3000".to_string())]);

    let env = build_worker_env("shop", "web", copy_one(), &base, &paths).unwrap();

    assert_eq!(env["PORT"], "3000");
    assert!(
        !persisted(&paths).contains_key("RIKU_UPSTREAM_PORT"),
        "a persisted port lets the last copy win: {:?}",
        persisted(&paths)
    );
}

#[test]
fn custom_socket_is_where_worker_listens() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let base = HashMap::from([("RIKU_NGINX_SOCKET".to_string(), "/srv/s.sock".to_string())]);

    let env = build_worker_env("shop", "wsgi", copy_one(), &base, &paths).unwrap();

    assert_eq!(env["UWSGI_SOCKET"], "/srv/s.sock");
    assert_eq!(env["SOCKET"], "unix:///srv/s.sock");
}

#[test]
fn each_wsgi_copy_listens_on_its_own_socket() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let second = CopyIndex::new(2).unwrap();

    let env = build_worker_env("shop", "wsgi", second, &HashMap::new(), &paths).unwrap();

    let expected = paths.nginx_root.join("shop.2.sock");
    assert_eq!(env["UWSGI_SOCKET"], expected.to_str().unwrap());
}

#[test]
fn rejects_a_pinned_port_that_is_not_a_port() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let base = HashMap::from([("PORT".to_string(), "$PORT".to_string())]);

    let err = build_worker_env("shop", "web", copy_one(), &base, &paths).unwrap_err();

    assert!(err.to_string().contains("$PORT"), "{err}");
}
