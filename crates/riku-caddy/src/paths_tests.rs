use riku_util::backend::Backend;

use super::*;

fn shop() -> (String, Vec<Backend>) {
    (
        "shop".to_string(),
        vec![Backend::loopback(5001), Backend::loopback(5002)],
    )
}

#[test]
fn caddy_paths_strip_prefix() {
    let site = render_paths(8080, &[shop()]).unwrap();

    assert!(
        site.starts_with("# riku path apps\nhttp://:8080 {\n"),
        "{site}"
    );
    assert!(site.contains("\thandle_path /shop/* {\n"), "{site}");
    assert!(
        site.contains("\t\treverse_proxy 127.0.0.1:5001 127.0.0.1:5002 {\n\t\t\theader_up X-Forwarded-Prefix /shop\n\t\t}\n"),
        "{site}"
    );
}

#[test]
fn bare_app_path_redirects_to_slash() {
    let site = render_paths(8080, &[shop()]).unwrap();

    assert!(site.contains("\tredir /shop /shop/\n"), "{site}");
}

#[test]
fn a_path_of_no_app_is_not_found() {
    let site = render_paths(8080, &[shop()]).unwrap();

    assert!(
        site.ends_with("\thandle {\n\t\trespond 404\n\t}\n}\n"),
        "without it Caddy answers an empty 200: {site}"
    );
}

#[test]
fn unix_backend_is_refused() {
    let unix = (
        "shop".to_string(),
        vec![Backend::Unix("/run/riku-sockets/shop.sock".into())],
    );

    assert!(render_paths(8080, &[unix]).is_err());
}
