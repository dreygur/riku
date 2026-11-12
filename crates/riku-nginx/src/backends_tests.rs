use super::*;

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn backends(pairs: &[(&str, &str)]) -> anyhow::Result<Vec<Backend>> {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = crate::config::RikuPaths::for_tests(temp_dir.path());
    default_backends("myapp", &env(pairs), &paths)
}

#[test]
fn default_backend_follows_portmap_and_socket() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = crate::config::RikuPaths::for_tests(temp_dir.path());

    assert_eq!(
        backends(&[("RIKU_NGINX_PORTMAP", "true"), ("PORT", "5000")]).unwrap(),
        vec![Backend::loopback(5000)]
    );
    assert_eq!(
        backends(&[
            ("RIKU_NGINX_PORTMAP", "true"),
            ("RIKU_UPSTREAM_PORT", "5001"),
            ("PORT", "5000"),
        ])
        .unwrap(),
        vec![Backend::loopback(5001)]
    );
    assert_eq!(
        default_backends("myapp", &HashMap::new(), &paths).unwrap(),
        vec![Backend::Unix(paths.socket_root.join("myapp.sock"))]
    );
}

#[test]
fn portmap_without_a_port_key_defaults_to_8080() {
    assert_eq!(
        backends(&[("RIKU_NGINX_PORTMAP", "true")]).unwrap(),
        vec![Backend::loopback(8080)]
    );
}

#[test]
fn portmap_backend_takes_its_host_from_the_bind_address() {
    assert_eq!(
        backends(&[
            ("RIKU_NGINX_PORTMAP", "true"),
            ("RIKU_BIND_ADDRESS", "10.0.0.5"),
            ("PORT", "5001"),
        ])
        .unwrap(),
        vec![Backend::Tcp {
            host: "10.0.0.5".parse().unwrap(),
            port: 5001
        }]
    );
}

#[test]
fn bind_address_that_is_not_an_ip_is_refused() {
    let err = backends(&[
        ("RIKU_NGINX_PORTMAP", "true"),
        ("RIKU_BIND_ADDRESS", "my-host"),
    ])
    .unwrap_err()
    .to_string();

    assert!(
        err.contains("RIKU_BIND_ADDRESS") && err.contains("my-host"),
        "error does not name the key and value: {err}"
    );
}

#[test]
fn port_that_is_not_a_port_number_is_refused_not_defaulted() {
    let err = backends(&[
        ("RIKU_NGINX_PORTMAP", "true"),
        ("RIKU_UPSTREAM_PORT", "http"),
        ("PORT", "5000"),
    ])
    .unwrap_err()
    .to_string();

    assert!(
        err.contains("RIKU_UPSTREAM_PORT") && err.contains("http"),
        "error does not name the key and value: {err}"
    );
}
