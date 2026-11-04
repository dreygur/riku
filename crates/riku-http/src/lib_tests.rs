use super::*;

#[test]
fn builds_a_client_more_than_once_with_ring_installed() {
    for _ in 0..2 {
        let built = blocking_client_builder().build();
        assert!(built.is_ok(), "client build failed: {:?}", built.err());
    }
    assert!(
        rustls::crypto::CryptoProvider::get_default().is_some(),
        "no rustls provider installed"
    );
}
