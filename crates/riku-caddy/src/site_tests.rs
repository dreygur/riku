use riku_util::backend::Backend;
use riku_util::domain::parse_domains;

use super::*;

fn plan(https: HttpsPolicy, backends: Vec<Backend>) -> RoutePlan {
    RoutePlan {
        domains: parse_domains("shop.example.com www.shop.example.com").unwrap(),
        https,
        backends,
    }
}

fn request(https: HttpsPolicy) -> RoutePlan {
    plan(https, vec![Backend::Tcp(5000)])
}

#[test]
fn https_only_site() {
    let expected = "# riku app: shop\n\
        shop.example.com, www.shop.example.com {\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n";

    assert_eq!(
        render_site("shop", &request(HttpsPolicy::Only)).unwrap(),
        expected
    );
}

#[test]
fn plain_site_adds_http() {
    let expected = "# riku app: shop\n\
        shop.example.com, www.shop.example.com, http://shop.example.com, http://www.shop.example.com {\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n";

    assert_eq!(
        render_site("shop", &request(HttpsPolicy::AlsoPlainHttp)).unwrap(),
        expected
    );
}

#[test]
fn every_backend_is_an_upstream() {
    let plan = plan(
        HttpsPolicy::Only,
        vec![Backend::Tcp(5001), Backend::Tcp(5002)],
    );

    assert!(render_site("shop", &plan)
        .unwrap()
        .contains("\treverse_proxy 127.0.0.1:5001 127.0.0.1:5002\n"));
}

#[test]
fn socket_backend_is_refused() {
    let plan = plan(HttpsPolicy::Only, vec![Backend::Unix("/x.sock".into())]);

    let err = render_site("shop", &plan).unwrap_err();

    assert!(err.to_string().contains("TCP only"), "{err}");
}

#[test]
fn empty_backend_list_is_refused() {
    let err = render_site("shop", &plan(HttpsPolicy::Only, vec![])).unwrap_err();

    assert!(err.to_string().contains("no backend"), "{err}");
}

#[test]
fn write_then_remove_site() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let site_file = paths.caddy_root.join("shop.caddy");

    write_site("shop", &request(HttpsPolicy::Only), &paths).unwrap();
    assert!(site_file.exists(), "site file missing at {site_file:?}");

    remove_site("shop", &paths).unwrap();
    assert!(!site_file.exists(), "site file left at {site_file:?}");
    remove_site("shop", &paths).unwrap();
}
