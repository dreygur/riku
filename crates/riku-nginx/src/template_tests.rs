use super::*;
use crate::backends::default_backends;

const SOCKET: &str = "/srv/riku/myapp.sock";

// select_template

#[test]
fn test_select_template_default_when_no_flags() {
    let env = HashMap::new();
    assert_eq!(select_template(&env), "nginx.conf.tera");
}

#[test]
fn test_select_template_https_only() {
    let mut env = HashMap::new();
    env.insert("RIKU_HTTPS_ONLY".to_string(), "true".to_string());
    assert_eq!(select_template(&env), "nginx_https_only.conf.tera");
}

#[test]
fn test_select_template_wsgi() {
    let mut env = HashMap::new();
    env.insert("RIKU_NGINX_WSGI".to_string(), "true".to_string());
    assert_eq!(select_template(&env), "nginx_wsgi.conf.tera");
}

#[test]
fn test_select_template_portmap() {
    let mut env = HashMap::new();
    env.insert("RIKU_NGINX_PORTMAP".to_string(), "true".to_string());
    assert_eq!(select_template(&env), "nginx_portmap.conf.tera");
}

#[test]
fn test_select_template_static() {
    let mut env = HashMap::new();
    env.insert("RIKU_NGINX_STATIC".to_string(), "true".to_string());
    assert_eq!(select_template(&env), "nginx_static.conf.tera");
}

#[test]
fn test_select_template_https_only_takes_precedence_over_wsgi() {
    let mut env = HashMap::new();
    env.insert("RIKU_HTTPS_ONLY".to_string(), "true".to_string());
    env.insert("RIKU_NGINX_WSGI".to_string(), "true".to_string());
    // HTTPS_ONLY is checked first
    assert_eq!(select_template(&env), "nginx_https_only.conf.tera");
}

// register_templates

#[test]
fn test_register_templates_succeeds() {
    let mut tera = tera::Tera::default();
    let result = register_templates(&mut tera);
    assert!(result.is_ok());
}

#[test]
fn test_all_expected_templates_registered() {
    let mut tera = tera::Tera::default();
    register_templates(&mut tera).unwrap();
    let names: Vec<String> = tera.get_template_names().map(str::to_string).collect();
    assert!(names.contains(&"nginx.conf.tera".to_string()));
    assert!(names.contains(&"nginx_https_only.conf.tera".to_string()));
    assert!(names.contains(&"nginx_portmap.conf.tera".to_string()));
    assert!(names.contains(&"nginx_static.conf.tera".to_string()));
    assert!(names.contains(&"nginx_wsgi.conf.tera".to_string()));
}

/// Renders the config `generate_nginx_config` would write for `pairs`, with
/// the backends `default_backends` picks for that ENV.
/// No `RIKU_DOMAINS`, so an HTTPS only render skips certificate issuance.
fn render(pairs: &[(&str, &str)]) -> String {
    render_with(pairs, None)
}

/// Like [`render`], with an explicit backend list when `backends` is `Some`.
fn render_with(pairs: &[(&str, &str)], backends: Option<&[Backend]>) -> String {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = crate::config::RikuPaths::for_tests(temp_dir.path());
    let env: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let default = default_backends("myapp", &env, &paths).unwrap();

    render_nginx_config(
        "myapp",
        temp_dir.path(),
        &env,
        backends.unwrap_or(&default),
        &paths,
    )
    .unwrap()
}

/// nginx rejects a bare socket path as an upstream ("invalid host in
/// upstream"), so every use of the socket must carry the `unix:` prefix.
fn assert_socket_always_prefixed(config: &str) {
    let uses = config.matches(SOCKET).count();
    let prefixed = config.matches(&format!("unix:{SOCKET}")).count();
    assert_eq!(uses, prefixed, "bare socket path in config:\n{config}");
}

#[test]
fn portmap_proxies_to_every_backend() {
    let config = render_with(
        &[("RIKU_NGINX_PORTMAP", "true")],
        Some(&[Backend::loopback(5001), Backend::loopback(5002)]),
    );

    assert!(
        config.contains(
            "upstream riku_6d79617070 {\n  server 127.0.0.1:5001;\n  server 127.0.0.1:5002;\n}"
        ),
        "upstream block missing or malformed:\n{config}"
    );
    assert!(
        config.contains("proxy_pass http://riku_6d79617070;"),
        "location not proxied to the upstream:\n{config}"
    );
}

#[test]
fn apps_differing_only_in_case_get_distinct_upstreams() {
    // nginx compares upstream names case-insensitively, so `MyApp_UPSTREAM`
    // and `myapp_UPSTREAM` would be a duplicate and fail every reload.
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = crate::config::RikuPaths::for_tests(temp_dir.path());
    let env = HashMap::from([("RIKU_NGINX_PORTMAP".to_string(), "true".to_string())]);
    let backends = [Backend::loopback(5001)];
    let render_for =
        |app: &str| render_nginx_config(app, temp_dir.path(), &env, &backends, &paths).unwrap();

    let upper = render_for("MyApp");
    let lower = render_for("myapp");

    assert!(
        upper.contains("upstream riku_4d79417070 {"),
        "MyApp:\n{upper}"
    );
    assert!(
        lower.contains("upstream riku_6d79617070 {"),
        "myapp:\n{lower}"
    );
    assert!(
        upper.contains("proxy_pass http://riku_4d79417070;"),
        "MyApp not proxied to its own upstream:\n{upper}"
    );
}

#[test]
fn https_only_web_app_proxies_to_its_ports() {
    let config = render_with(
        &[("RIKU_HTTPS_ONLY", "true"), ("RIKU_NGINX_PORTMAP", "true")],
        Some(&[Backend::loopback(5001), Backend::loopback(5002)]),
    );

    assert!(config.contains("443 ssl"), "not an HTTPS config:\n{config}");
    assert!(
        config.contains("server 127.0.0.1:5001;") && config.contains("server 127.0.0.1:5002;"),
        "upstream does not list both backends:\n{config}"
    );
    assert!(
        config.contains("proxy_pass http://riku_6d79617070;"),
        "web app not proxied to its upstream:\n{config}"
    );
    assert!(
        config.find("upstream riku_6d79617070") < config.find("server {"),
        "upstream must sit outside the server blocks:\n{config}"
    );
}

#[test]
fn app_env_cannot_override_the_generated_names() {
    let config = render_with(
        &[
            ("RIKU_HTTPS_ONLY", "true"),
            ("RIKU_NGINX_PORTMAP", "true"),
            ("APP_UPSTREAM", "proxy"),
            ("APP", "shop"),
        ],
        Some(&[Backend::loopback(5001)]),
    );

    assert!(
        config.contains("upstream riku_6d79617070 {")
            && config.contains("proxy_pass http://riku_6d79617070;"),
        "ENV replaced the generated upstream name:\n{config}"
    );
    assert!(
        config.contains("/myapp.crt;") && !config.contains("shop"),
        "ENV replaced the app name:\n{config}"
    );
}

#[test]
fn https_only_socket_app_never_uses_a_bare_socket_path() {
    let config = render(&[("RIKU_HTTPS_ONLY", "true"), ("RIKU_NGINX_SOCKET", SOCKET)]);

    assert!(
        config.contains(&format!("server unix:{SOCKET};")),
        "upstream does not name the socket:\n{config}"
    );
    assert!(
        config.contains("proxy_pass http://riku_6d79617070;"),
        "socket app not proxied to its upstream:\n{config}"
    );
    assert_socket_always_prefixed(&config);
}

#[test]
fn backend_with_nginx_directive_characters_is_refused() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = crate::config::RikuPaths::for_tests(temp_dir.path());
    let hostile = [Backend::Unix("/s.sock; include /etc/passwd".into())];

    let rendered = render_nginx_config("myapp", temp_dir.path(), &HashMap::new(), &hostile, &paths);

    assert!(rendered.is_err(), "hostile backend was rendered");
}

#[test]
fn socket_backend_with_whitespace_is_refused() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = crate::config::RikuPaths::for_tests(temp_dir.path());
    // A space would let "weight=0" be read as a server parameter.
    let padded = [Backend::Unix("/x.sock weight=0".into())];

    let rendered = render_nginx_config("myapp", temp_dir.path(), &HashMap::new(), &padded, &paths);

    assert!(rendered.is_err(), "backend with a space was rendered");
}

#[test]
fn portmap_upstream_uses_the_bind_address() {
    let config = render(&[
        ("RIKU_NGINX_PORTMAP", "true"),
        ("RIKU_BIND_ADDRESS", "10.0.0.5"),
        ("RIKU_UPSTREAM_PORT", "5001"),
    ]);

    assert!(
        config.contains("server 10.0.0.5:5001;"),
        "upstream ignores RIKU_BIND_ADDRESS:\n{config}"
    );
}

#[test]
fn https_only_server_declares_the_acme_location_once() {
    let config = render(&[("RIKU_HTTPS_ONLY", "true"), ("RIKU_NGINX_SOCKET", SOCKET)]);
    let tls_server = match config.split_once("443 ssl") {
        Some((_, rest)) => rest,
        None => panic!("not an HTTPS config:\n{config}"),
    };

    // nginx refuses to start on a duplicate location in one server block.
    assert_eq!(
        tls_server
            .matches("location ^~ /.well-known/acme-challenge")
            .count(),
        1,
        "TLS server block:\n{tls_server}"
    );
}

#[test]
fn https_only_wsgi_app_passes_to_its_uwsgi_upstream() {
    let config = render_with(
        &[("RIKU_HTTPS_ONLY", "true"), ("RIKU_NGINX_WSGI", "true")],
        Some(&[Backend::Unix(SOCKET.into())]),
    );

    assert!(config.contains("443 ssl"), "not an HTTPS config:\n{config}");
    assert_socket_always_prefixed(&config);
    assert!(
        config.contains("uwsgi_pass riku_6d79617070;"),
        "wsgi app not passed to its upstream:\n{config}"
    );
    assert!(
        !config.contains("proxy_pass"),
        "wsgi app proxied over http:\n{config}"
    );
}

#[test]
fn wsgi_app_balances_over_every_socket_backend() {
    let second = "/srv/riku/myapp.2.sock";
    let config = render_with(
        &[("RIKU_NGINX_WSGI", "true")],
        Some(&[Backend::Unix(SOCKET.into()), Backend::Unix(second.into())]),
    );

    assert!(
        config.contains(&format!(
            "upstream riku_6d79617070 {{\n  server unix:{SOCKET};\n  server unix:{second};\n}}"
        )),
        "upstream block missing or malformed:\n{config}"
    );
    assert!(
        config.contains("uwsgi_pass riku_6d79617070;"),
        "no uwsgi_pass to the upstream:\n{config}"
    );
}

#[test]
fn https_only_false_keeps_plain_template() {
    let env = HashMap::from([("RIKU_HTTPS_ONLY".to_string(), "false".to_string())]);
    assert_eq!(select_template(&env), "nginx.conf.tera");
}
