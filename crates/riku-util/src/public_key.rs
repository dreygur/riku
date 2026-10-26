//! SSH public keys read from a `.pub` or `authorized_keys` file.
//!
//! Security: a key ends up after riku's own `command=` options in the deploy
//! user's `authorized_keys`, so anything kept from the input line could add
//! options or a second key. Only the key type, from a fixed list, and the
//! base64 blob survive parsing; options and comments are dropped.

use std::fmt;

use anyhow::{anyhow, Result};

const KEY_TYPES: [&str; 8] = [
    "ssh-ed25519",
    "ssh-rsa",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
    "sk-ssh-ed25519@openssh.com",
    "sk-ecdsa-sha2-nistp256@openssh.com",
    "ssh-dss",
];

/// A validated public key: a known type and a base64 blob, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    kind: &'static str,
    blob: String,
}

impl PublicKey {
    /// The base64 key material, which identifies the key.
    pub fn blob(&self) -> &str {
        &self.blob
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind, self.blob)
    }
}

fn known_type(token: &str) -> Option<&'static str> {
    KEY_TYPES.iter().copied().find(|kind| *kind == token)
}

fn is_base64(blob: &str) -> bool {
    !blob.is_empty()
        && blob
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

/// `line` with a leading `authorized_keys` options field removed. Options
/// run to the first whitespace outside double quotes, as sshd reads them.
fn skip_options(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => return line[i..].trim_start(),
            _ => {}
        }
    }
    ""
}

impl TryFrom<&str> for PublicKey {
    type Error = anyhow::Error;

    fn try_from(line: &str) -> Result<Self> {
        let line = line.trim();
        let first = line.split_whitespace().next().unwrap_or_default();
        let key_part = match known_type(first) {
            Some(_) => line,
            None => skip_options(line),
        };

        let mut tokens = key_part.split_whitespace();
        let Some(kind) = tokens.next().and_then(known_type) else {
            return Err(anyhow!("not a supported SSH public key"));
        };
        match tokens.next() {
            Some(blob) if is_base64(blob) => Ok(Self {
                kind,
                blob: blob.to_string(),
            }),
            _ => Err(anyhow!("{kind} key has no valid key data")),
        }
    }
}

/// Every key in a `.pub` or `authorized_keys` file. Blank lines and `#`
/// comments are skipped; any other line that is not a key is an error
/// naming its line number, so a wrong file fails instead of adding nothing.
pub fn parse_public_keys(text: &str) -> Result<Vec<PublicKey>> {
    let mut keys = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match PublicKey::try_from(line) {
            Ok(key) => keys.push(key),
            Err(e) => return Err(anyhow!("line {}: {e}", index + 1)),
        }
    }
    if keys.is_empty() {
        return Err(anyhow!("no SSH public keys found"));
    }
    Ok(keys)
}

#[cfg(test)]
#[path = "public_key_tests.rs"]
mod tests;
