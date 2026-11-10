use super::*;
use crate::own_cert::test_pems::{OLD_KEY, VALID_CERT, VALID_KEY};

#[test]
fn accepts_the_chain_and_key_in_either_order() {
    let chain_first = validate(&format!("{VALID_CERT}{VALID_KEY}")).unwrap();
    let key_first = validate(&format!("{VALID_KEY}\n{VALID_CERT}")).unwrap();

    assert_eq!(chain_first.cert_pem, key_first.cert_pem);
    assert!(chain_first.cert_pem.contains("BEGIN CERTIFICATE"));
    assert!(chain_first.key_pem.contains("PRIVATE KEY"));
}

#[test]
fn leaf_is_the_first_certificate_of_the_chain() {
    let bundle = validate(&format!("{VALID_CERT}{VALID_CERT}{VALID_KEY}")).unwrap();

    assert_eq!(bundle.cert_pem.matches("BEGIN CERTIFICATE").count(), 2);
    assert_eq!(
        first_certificate(&bundle.cert_pem).matches("BEGIN").count(),
        1
    );
}

#[test]
fn refuses_a_key_that_belongs_to_another_certificate() {
    let err = validate(&format!("{VALID_CERT}{OLD_KEY}")).unwrap_err();

    assert!(err.to_string().contains("match"), "got: {err:#}");
}

#[test]
fn refuses_a_bundle_without_a_key_or_without_a_certificate() {
    assert!(validate(VALID_CERT).is_err(), "no key");
    assert!(validate(VALID_KEY).is_err(), "no certificate");
    assert!(validate("hello").is_err(), "no PEM at all");
}

#[test]
fn errors_never_contain_the_key() {
    let err = validate(&format!("{VALID_CERT}{VALID_KEY}{VALID_KEY}")).unwrap_err();
    let body = VALID_KEY.lines().nth(1).unwrap();

    assert!(!format!("{err:#}").contains(body), "error leaked the key");
}
