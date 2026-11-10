use super::*;
use crate::own_cert::test_pems::{OLD_CERT, VALID_CERT};

const DAY: i64 = 86_400;

#[test]
fn parses_names_and_expiry_from_openssl_output() {
    let output = "notAfter=Jan  2 00:00:00 2020 GMT\n\
        X509v3 Subject Alternative Name: \n    DNS:edge.example, DNS:*.shop.example\n";

    let info = parse_openssl(output).unwrap();

    assert_eq!(info.names, ["edge.example", "*.shop.example"]);
    assert_eq!(info.expires_at, 1_577_923_200);
}

#[test]
fn output_without_an_end_date_is_an_error() {
    assert!(parse_openssl("X509v3 Subject Alternative Name:\n").is_err());
}

#[test]
fn reads_a_real_certificate_through_openssl() {
    let valid = read(VALID_CERT).unwrap();
    let old = read(OLD_CERT).unwrap();

    assert_eq!(valid.names, ["edge.example", "*.shop.example"]);
    assert!(valid.expires_at > now());
    assert_eq!(old.expires_at, 1_577_923_200);
}

#[test]
fn a_wildcard_covers_one_label_only() {
    let names = vec!["edge.example".to_string(), "*.shop.example".to_string()];

    assert!(covers(&names, "edge.example"));
    assert!(covers(&names, "EDGE.example"));
    assert!(covers(&names, "www.shop.example"));
    assert!(!covers(&names, "a.b.shop.example"));
    assert!(!covers(&names, "shop.example"));
    assert!(!covers(&names, "other.example"));
}

#[test]
fn expiry_is_classified_at_the_fourteen_day_boundary() {
    let at = 1_000_000_000;

    assert_eq!(classify(at - 1, at), Expiry::Expired);
    assert_eq!(classify(at + 13 * DAY, at), Expiry::Soon { days: 13 });
    assert_eq!(classify(at + 14 * DAY, at), Expiry::Valid { days: 14 });
}
