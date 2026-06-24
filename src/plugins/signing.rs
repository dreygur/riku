//! Plugin author signatures (ROADMAP E2.5) — Ed25519 over the entry bytes.
//!
//! An author signs a bundle's entry executable with their secret key; the
//! operator adds the author's public key to a **trust keyring**. On install, a
//! signed bundle is accepted only if some trusted key verifies the signature —
//! so a signed plugin from an unknown publisher is rejected, not merely warned.
//! Keys and signatures are hex-encoded.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::config::RikuPaths;

/// An Ed25519 keypair for signing plugin bundles.
pub struct Keypair {
    signing: SigningKey,
}

impl Keypair {
    /// Generate a fresh keypair from the OS CSPRNG.
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut rand::rngs::OsRng),
        }
    }

    /// Restore a keypair from its 32-byte secret (hex).
    pub fn from_secret_hex(secret_hex: &str) -> Result<Self> {
        let bytes = hex::decode(secret_hex.trim()).context("decoding secret key")?;
        let arr: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow!("secret key must be 32 bytes"))?;
        Ok(Self {
            signing: SigningKey::from_bytes(&arr),
        })
    }

    pub fn public_hex(&self) -> String {
        hex::encode(self.signing.verifying_key().to_bytes())
    }

    pub fn secret_hex(&self) -> String {
        hex::encode(self.signing.to_bytes())
    }

    /// Sign `message`, returning the hex signature.
    pub fn sign_hex(&self, message: &[u8]) -> String {
        hex::encode(self.signing.sign(message).to_bytes())
    }
}

/// Verify a hex Ed25519 `signature` over `message` with a hex `pubkey`.
/// Any decode/length/verification failure returns `false`.
pub fn verify(message: &[u8], signature: &str, pubkey: &str) -> bool {
    let Some(vk) = decode_pubkey(pubkey) else {
        return false;
    };
    let Ok(sig_bytes) = hex::decode(signature.trim()) else {
        return false;
    };
    let Ok(sig_arr): std::result::Result<[u8; 64], _> = sig_bytes.as_slice().try_into() else {
        return false;
    };
    vk.verify_strict(message, &Signature::from_bytes(&sig_arr))
        .is_ok()
}

fn decode_pubkey(pubkey: &str) -> Option<VerifyingKey> {
    let bytes = hex::decode(pubkey.trim()).ok()?;
    let arr: [u8; 32] = bytes.as_slice().try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

/// A trusted publisher key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedKey {
    pub name: String,
    pub pubkey: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct KeyringDoc {
    #[serde(default, rename = "key")]
    keys: Vec<TrustedKey>,
}

/// Repository for the operator's trusted publisher keys
/// (`~/.riku/trusted-keys.toml`).
pub struct Keyring<'a> {
    paths: &'a RikuPaths,
}

impl<'a> Keyring<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    fn file(&self) -> PathBuf {
        self.paths.riku_root.join("trusted-keys.toml")
    }

    fn load(&self) -> KeyringDoc {
        std::fs::read_to_string(self.file())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, doc: &KeyringDoc) -> Result<()> {
        std::fs::create_dir_all(self.paths.riku_root.as_path())?;
        crate::util::write_atomic(&self.file(), toml::to_string_pretty(doc)?.as_bytes())
    }

    pub fn list(&self) -> Vec<TrustedKey> {
        let mut keys = self.load().keys;
        keys.sort_by(|a, b| a.name.cmp(&b.name));
        keys
    }

    /// Trust `pubkey` under `name`. Rejects a malformed key.
    pub fn add(&self, name: &str, pubkey: &str) -> Result<()> {
        if decode_pubkey(pubkey).is_none() {
            bail!("'{pubkey}' is not a valid 32-byte hex Ed25519 public key");
        }
        let mut doc = self.load();
        doc.keys.retain(|k| k.name != name);
        doc.keys.push(TrustedKey {
            name: name.to_string(),
            pubkey: pubkey.trim().to_string(),
        });
        self.save(&doc)
    }

    pub fn remove(&self, name: &str) -> Result<bool> {
        let mut doc = self.load();
        let before = doc.keys.len();
        doc.keys.retain(|k| k.name != name);
        let removed = doc.keys.len() != before;
        self.save(&doc)?;
        Ok(removed)
    }

    /// The trusted key whose public key verifies `signature` over `message`.
    pub fn verifier_of(&self, message: &[u8], signature: &str) -> Option<TrustedKey> {
        self.list()
            .into_iter()
            .find(|k| verify(message, signature, &k.pubkey))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (tempfile::TempDir, RikuPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        (tmp, paths)
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let kp = Keypair::generate();
        let msg = b"plugin entry bytes";
        let sig = kp.sign_hex(msg);
        assert!(verify(msg, &sig, &kp.public_hex()));
        // Wrong message / key / signature all fail.
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

        // An untrusted signer is not found.
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
}
