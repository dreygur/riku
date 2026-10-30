//! Ed25519 signatures over a plugin bundle's entry bytes.
//!
//! An author signs the entry executable with their secret key and puts the
//! hex signature in the manifest. Keys and signatures are hex encoded.
//!
//! Security model: a valid signature proves that the holder of the matching
//! secret key signed exactly these entry bytes. It does not prove the key
//! belongs to anyone in particular, and it covers only the entry file, not
//! the rest of the bundle. Deciding whether to trust a public key is the
//! caller's job (the server keeps a keyring for that).

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

/// An Ed25519 keypair for signing plugin bundles.
pub struct Keypair {
    signing: SigningKey,
}

impl Keypair {
    /// Generate a fresh keypair from the OS CSPRNG.
    ///
    /// Panics if the OS RNG fails, as `OsRng` did before rand 0.10 made it
    /// fallible. A key from any weaker source is worse than no key.
    pub fn generate() -> Self {
        let mut os_rng = rand::rand_core::UnwrapErr(rand::rngs::SysRng);
        Self {
            signing: SigningKey::generate(&mut os_rng),
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

    /// The public key as hex, the form a server's keyring stores.
    pub fn public_hex(&self) -> String {
        hex::encode(self.signing.verifying_key().to_bytes())
    }

    /// The secret key as hex. Keep it out of logs and shared files.
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

/// Decodes a hex public key, or `None` if it is not a valid 32-byte Ed25519 key.
pub fn decode_pubkey(pubkey: &str) -> Option<VerifyingKey> {
    let bytes = hex::decode(pubkey.trim()).ok()?;
    let arr: [u8; 32] = bytes.as_slice().try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

#[cfg(test)]
#[path = "signing_tests.rs"]
mod tests;
