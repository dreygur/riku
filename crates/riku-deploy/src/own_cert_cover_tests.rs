use std::fs;

use super::*;

/// A throwaway self-signed certificate for edge.example and *.shop.example.
const CERT: &str = "-----BEGIN CERTIFICATE-----
MIIBjDCCATOgAwIBAgIUfE3b/8moaWakpuBXefIfwej93rgwCgYIKoZIzj0EAwIw
FzEVMBMGA1UEAwwMZWRnZS5leGFtcGxlMCAXDTI2MDEwMTAwMDAwMFoYDzIxMjYw
MTAxMDAwMDAwWjAXMRUwEwYDVQQDDAxlZGdlLmV4YW1wbGUwWTATBgcqhkjOPQIB
BggqhkjOPQMBBwNCAATJYCyJ1ExAKtfM1lWxBK6Ch0US2YH33emtP2hHGiYbcIMV
PhfadyISYAbPKzMgFZt9Z/hk0Nlkd2kvsSNAgET6o1swWTAdBgNVHQ4EFgQUzz2u
iPR5jrgiqL6HXw/4AnIVBC8wDwYDVR0TAQH/BAUwAwEB/zAnBgNVHREEIDAeggxl
ZGdlLmV4YW1wbGWCDiouc2hvcC5leGFtcGxlMAoGCCqGSM49BAMCA0cAMEQCICkn
6AHZzPrab7fH3yaq0kaqT4He47BRvbrtNBt+7R/sAiBXU1XttFNMbA0Ius78FuU3
SK9PYpPVnT4WntKycpqStA==
-----END CERTIFICATE-----
";

fn paths_with_chain(chain: &str) -> (tempfile::TempDir, RikuPaths) {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = RikuPaths::for_tests(dir.path());
    fs::create_dir_all(&paths.nginx_root).expect("nginx root");
    fs::write(paths.nginx_root.join("shop.fullchain.crt"), chain).expect("chain");
    (dir, paths)
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| name.to_string()).collect()
}

#[test]
fn keeps_the_domains_the_certificate_names_or_matches_by_wildcard() {
    let (_dir, paths) = paths_with_chain(CERT);
    let domains = names(&[
        "edge.example",
        "a.shop.example",
        "a.b.shop.example",
        "other.example",
        "*.shop.example",
        "203.0.113.5",
    ]);

    assert_eq!(
        covered_domains(&paths, "shop", &domains),
        names(&["edge.example", "a.shop.example"])
    );
}

#[test]
fn an_unreadable_chain_covers_nothing() {
    let (_dir, paths) = paths_with_chain("not a certificate");

    assert!(covered_domains(&paths, "shop", &names(&["edge.example"])).is_empty());
    assert!(covered_domains(&paths, "missing", &names(&["edge.example"])).is_empty());
}
