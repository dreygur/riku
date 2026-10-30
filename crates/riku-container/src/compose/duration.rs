//! Compose duration strings.

use anyhow::{anyhow, Result};
use std::time::Duration;

/// Parse `30s`, `1m30s`, `250ms` and similar. Units are `us`, `ms`, `s`, `m`
/// and `h`; amounts are whole numbers.
pub(crate) fn parse_duration(raw: &str) -> Result<Duration> {
    let invalid = || anyhow!("invalid duration {raw:?}: expected a value like 30s, 1m30s or 500ms");
    if raw.is_empty() {
        return Err(invalid());
    }
    let mut rest = raw;
    let mut total = Duration::ZERO;
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits == 0 || digits == rest.len() {
            return Err(invalid());
        }
        let amount: u64 = match rest[..digits].parse() {
            Ok(amount) => amount,
            Err(_) => return Err(invalid()),
        };
        rest = &rest[digits..];
        let unit_len = rest
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(rest.len());
        let part = match &rest[..unit_len] {
            "us" => Some(Duration::from_micros(amount)),
            "ms" => Some(Duration::from_millis(amount)),
            "s" => Some(Duration::from_secs(amount)),
            "m" => amount.checked_mul(60).map(Duration::from_secs),
            "h" => amount.checked_mul(3600).map(Duration::from_secs),
            _ => None,
        };
        rest = &rest[unit_len..];
        total = match part.and_then(|part| total.checked_add(part)) {
            Some(total) => total,
            None => return Err(invalid()),
        };
    }
    Ok(total)
}

#[cfg(test)]
#[path = "duration_tests.rs"]
mod tests;
