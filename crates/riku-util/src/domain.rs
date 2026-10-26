//! Validated hostnames.
//!
//! Security model: `RIKU_DOMAINS` comes from the app ENV and ends up inside
//! router config files. A [`Domain`] only holds lowercase letters, digits,
//! hyphens, dots and an optional leading `*` label, so no value can carry
//! config syntax (`{`, `;`, whitespace, newlines) into a generated file.

use std::fmt;

use anyhow::{anyhow, Result};

const MAX_DOMAIN_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;
const WILDCARD: &str = "*";

/// A hostname that passed validation, always lowercase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Domain(String);

impl Domain {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn valid_label(label: &str) -> bool {
    let chars_ok = label
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');

    !label.is_empty()
        && label.len() <= MAX_LABEL_LEN
        && chars_ok
        && !label.starts_with('-')
        && !label.ends_with('-')
}

impl TryFrom<&str> for Domain {
    type Error = anyhow::Error;

    fn try_from(raw: &str) -> Result<Self> {
        let host = raw.to_ascii_lowercase();
        if host.is_empty() || host.len() > MAX_DOMAIN_LEN {
            return Err(anyhow!(
                "invalid domain {raw:?}: length must be 1 to {MAX_DOMAIN_LEN}"
            ));
        }

        let all_valid = host
            .split('.')
            .enumerate()
            .all(|(i, label)| (i == 0 && label == WILDCARD) || valid_label(label));
        if !all_valid {
            return Err(anyhow!("invalid domain {raw:?}: not a hostname"));
        }

        Ok(Domain(host))
    }
}

/// Parse a `RIKU_DOMAINS` value: entries split on commas and whitespace,
/// empties skipped. Fails on the first invalid entry, naming both the entry
/// and the whole value, since splitting can cut a bad value apart.
pub fn parse_domains(raw: &str) -> Result<Vec<Domain>> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|entry| !entry.is_empty())
        .map(Domain::try_from)
        .collect::<Result<Vec<Domain>>>()
        .map_err(|e| anyhow!("{e} (in RIKU_DOMAINS={raw:?})"))
}

#[cfg(test)]
#[path = "domain_tests.rs"]
mod tests;
