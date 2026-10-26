use super::*;

#[test]
fn units_follow_router() {
    let caddy = render_system_unit(&Router::Caddy, "deploy", "/home/deploy", "/usr/bin/riku");
    assert!(
        caddy.contains("After=network.target caddy.service\nWants=caddy.service\n"),
        "{caddy}"
    );
    assert!(
        !caddy.contains("nginx"),
        "caddy host unit mentions nginx: {caddy}"
    );

    let nginx = render_system_unit(&Router::Nginx, "deploy", "/home/deploy", "/usr/bin/riku");
    assert!(
        nginx.contains("After=network.target nginx.service\nWants=nginx.service\n"),
        "{nginx}"
    );
}
