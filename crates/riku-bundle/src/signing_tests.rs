use super::*;

#[test]
fn sign_and_verify_roundtrip() {
    let kp = Keypair::generate();
    let msg = b"plugin entry bytes";
    let sig = kp.sign_hex(msg);
    assert!(verify(msg, &sig, &kp.public_hex()));
    assert!(!verify(b"tampered", &sig, &kp.public_hex()));
    assert!(!verify(msg, &sig, &Keypair::generate().public_hex()));
    assert!(!verify(msg, "deadbeef", &kp.public_hex()));
}

#[test]
fn secret_hex_roundtrip() {
    let kp = Keypair::generate();
    let restored = Keypair::from_secret_hex(&kp.secret_hex()).unwrap();
    assert_eq!(kp.public_hex(), restored.public_hex());
}
