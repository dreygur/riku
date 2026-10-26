use super::*;
use tempfile::TempDir;

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

    build_worker_env("shop", "wsgi", &base, &paths).unwrap();
    build_worker_env("shop", "web", &base, &paths).unwrap();
    let after_web = persisted(&paths);
    for key in WSGI_KEYS {
        assert!(
            !after_web.contains_key(key),
            "{key} left after wsgi to web: {after_web:?}"
        );
    }
    assert!(
        after_web.contains_key("RIKU_UPSTREAM_PORT"),
        "{after_web:?}"
    );

    build_worker_env("shop", "wsgi", &base, &paths).unwrap();
    let after_wsgi = persisted(&paths);
    for key in PORTMAP_KEYS {
        assert!(
            !after_wsgi.contains_key(key),
            "{key} left after web to wsgi: {after_wsgi:?}"
        );
    }
    assert!(after_wsgi.contains_key("RIKU_NGINX_WSGI"), "{after_wsgi:?}");
}
