use super::*;

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn default_backend_follows_portmap_and_socket() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = crate::config::RikuPaths::for_tests(temp_dir.path());

    let portmap = env(&[("RIKU_NGINX_PORTMAP", "true"), ("PORT", "5000")]);
    assert_eq!(
        default_backends("myapp", &portmap, &paths),
        vec![Backend::Tcp(5000)]
    );

    let upstream_wins = env(&[
        ("RIKU_NGINX_PORTMAP", "true"),
        ("RIKU_UPSTREAM_PORT", "5001"),
        ("PORT", "5000"),
    ]);
    assert_eq!(
        default_backends("myapp", &upstream_wins, &paths),
        vec![Backend::Tcp(5001)]
    );

    assert_eq!(
        default_backends("myapp", &HashMap::new(), &paths),
        vec![Backend::Unix(paths.nginx_root.join("myapp.sock"))]
    );

    let custom = env(&[("RIKU_NGINX_SOCKET", "/srv/riku/custom.sock")]);
    assert_eq!(
        default_backends("myapp", &custom, &paths),
        vec![Backend::Unix(PathBuf::from("/srv/riku/custom.sock"))]
    );
}

#[test]
fn portmap_without_a_port_falls_back_to_8080() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = crate::config::RikuPaths::for_tests(temp_dir.path());
    let portmap = env(&[("RIKU_NGINX_PORTMAP", "true")]);

    assert_eq!(
        default_backends("myapp", &portmap, &paths),
        vec![Backend::Tcp(8080)]
    );
}
