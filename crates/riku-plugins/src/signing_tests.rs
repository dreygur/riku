use super::*;

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
    (tmp, paths)
}

#[test]
fn keyring_finds_the_verifying_key() {
    let (_tmp, paths) = paths();
    let kp = Keypair::generate();
    let ring = Keyring::new(&paths);
    ring.add("acme", &kp.public_hex()).unwrap();
    ring.add("other", &Keypair::generate().public_hex())
        .unwrap();

    let msg = b"hello";
    let sig = kp.sign_hex(msg);
    assert_eq!(ring.verifier_of(msg, &sig).unwrap().name, "acme");

    let stranger = Keypair::generate();
    assert!(ring.verifier_of(msg, &stranger.sign_hex(msg)).is_none());
}

#[test]
fn keyring_rejects_bad_pubkey_and_removes() {
    let (_tmp, paths) = paths();
    let ring = Keyring::new(&paths);
    assert!(ring.add("bad", "not-hex").is_err());
    ring.add("ok", &Keypair::generate().public_hex()).unwrap();
    assert!(ring.remove("ok").unwrap());
    assert!(!ring.remove("ok").unwrap());
}
