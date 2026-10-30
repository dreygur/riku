use riku_util::domain::parse_domains;

use super::*;

fn request(https: HttpsPolicy) -> SiteRequest {
    SiteRequest {
        domains: parse_domains("shop.example.com www.shop.example.com").unwrap(),
        upstream_port: 5000,
        https,
    }
}

#[test]
fn https_only_site() {
    let expected = "# riku app: shop\n\
        shop.example.com, www.shop.example.com {\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n";

    assert_eq!(render_site("shop", &request(HttpsPolicy::Only)), expected);
}

#[test]
fn plain_site_adds_http() {
    let expected = "# riku app: shop\n\
        shop.example.com, www.shop.example.com, http://shop.example.com, http://www.shop.example.com {\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n";

    assert_eq!(
        render_site("shop", &request(HttpsPolicy::AlsoPlainHttp)),
        expected
    );
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
