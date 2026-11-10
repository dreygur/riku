//! Splitting one PEM bundle, as read from stdin, into a certificate chain
//! and a private key, and checking the two belong together.
//!
//! Security model: stdin is untrusted. Blocks are separated by their armor
//! label only, then `riku_route::check_tls` re-parses and re-encodes both
//! halves and proves the key signs for the first certificate, so nothing
//! outside well formed PEM sections reaches disk. No error text carries PEM
//! content.

use anyhow::{anyhow, Result};
use riku_route::Tls;

const END_MARKER: &str = "-----END ";

/// A validated pair, re-encoded from what was parsed.
pub(crate) struct Bundle {
    /// The chain, leaf first.
    pub cert_pem: String,
    /// The private key of the leaf.
    pub key_pem: String,
}

impl std::fmt::Debug for Bundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bundle")
            .field("cert_pem", &self.cert_pem)
            .field("key_pem", &"<redacted>")
            .finish()
    }
}

struct Blocks {
    certificates: String,
    keys: Vec<String>,
}

fn is_key_label(begin_line: &str) -> bool {
    begin_line.contains("PRIVATE KEY")
}

fn blocks(text: &str) -> Blocks {
    let mut found = Blocks {
        certificates: String::new(),
        keys: Vec::new(),
    };
    let mut current: Option<(bool, String)> = None;
    for line in text.lines().map(str::trim_end) {
        if line.starts_with("-----BEGIN ") && line.ends_with("-----") {
            current = Some((is_key_label(line), format!("{line}\n")));
        } else if let Some((is_key, mut block)) = current.take() {
            block.push_str(line);
            block.push('\n');
            if line.starts_with(END_MARKER) {
                if is_key {
                    found.keys.push(block);
                } else {
                    found.certificates.push_str(&block);
                }
            } else {
                current = Some((is_key, block));
            }
        }
    }
    found
}

/// The first certificate of `chain_pem`, which is the leaf.
pub(crate) fn first_certificate(chain_pem: &str) -> &str {
    match chain_pem.find("-----END CERTIFICATE-----\n") {
        Some(at) => &chain_pem[..at + "-----END CERTIFICATE-----\n".len()],
        None => chain_pem,
    }
}

/// Split `text` into chain and key and refuse it unless the key matches the
/// first certificate.
pub(crate) fn validate(text: &str) -> Result<Bundle> {
    let mut found = blocks(text);
    if found.certificates.is_empty() {
        return Err(anyhow!("the input holds no certificate"));
    }
    if found.keys.len() != 1 {
        return Err(anyhow!(
            "the input must hold exactly one private key, found {}",
            found.keys.len()
        ));
    }
    let pair = Tls {
        cert_pem: found.certificates,
        key_pem: found.keys.remove(0),
    };
    match riku_route::check_tls(&pair) {
        Ok(checked) => Ok(Bundle {
            cert_pem: checked.cert_pem,
            key_pem: checked.key_pem,
        }),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
