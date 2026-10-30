//! Plugin author signatures (ROADMAP E2.5): the operator's trust keyring.
//!
//! The operator adds an author's public key to a **trust keyring**. On install,
//! a signed bundle is accepted only if some trusted key verifies the
//! signature, so a signed plugin from an unknown publisher is rejected, not
//! merely warned. The keypair and signature check live in `riku-bundle`.

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::config::RikuPaths;

use riku_bundle::signing::decode_pubkey;
pub use riku_bundle::signing::{verify, Keypair};

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
#[path = "signing_tests.rs"]
mod tests;
