use super::*;
use crate::util::backend::Backend;

fn render(apps: &[PathApp]) -> String {
    render_paths_site(&PathsSite {
        port: 8080,
        ipv6: false,
        apps,
    })
    .unwrap()
}

fn shop() -> PathApp {
    PathApp {
        app: "shop".to_string(),
        backends: vec![Backend::loopback(5001), Backend::loopback(5002)],
    }
}

#[test]
fn nginx_paths_strip_prefix_and_send_forwarded_prefix() {
    let config = render(&[shop()]);

    assert!(
        config.contains("listen              0.0.0.0:8080;"),
        "{config}"
    );
    assert!(config.contains("location /shop/ {"), "{config}");
    assert!(
        config.contains("proxy_pass http://riku_73686f70_path/;"),
        "the trailing slash strips the prefix: {config}"
    );
    assert!(
        config.contains("proxy_set_header X-Forwarded-Prefix /shop;"),
        "{config}"
    );
    assert!(
        config.contains(
            "upstream riku_73686f70_path {\n  server 127.0.0.1:5001;\n  server 127.0.0.1:5002;\n}"
        ),
        "{config}"
    );
}

#[test]
fn bare_app_path_redirects_to_slash() {
    let config = render(&[shop()]);

    assert!(
        config.contains("location = /shop {\n    return 301 /shop/;\n  }"),
        "{config}"
    );
}

#[test]
fn every_app_gets_its_own_location() {
    let blog = PathApp {
        app: "blog".to_string(),
        backends: vec![Backend::loopback(5003)],
    };

    let config = render(&[shop(), blog]);

    assert!(
        config.contains("location /shop/ {") && config.contains("location /blog/ {"),
        "{config}"
    );
}

#[test]
fn app_name_with_directive_characters_is_refused() {
    let bad = PathApp {
        app: "shop;".to_string(),
        ..shop()
    };

    let result = render_paths_site(&PathsSite {
        port: 8080,
        ipv6: false,
        apps: &[bad],
    });

    assert!(result.is_err());
}
