//! Scalar coercion and text checks shared by the service field parsers.
//!
//! YAML numbers reach us as JSON numbers, so `1.10` has already become `1.1`.
//! Where a compose field holds text, integers and booleans are turned into
//! their plain text and any other number is refused, because it may not be
//! what the author typed. Text that ends up in an engine argument may not hold
//! a line break or a NUL byte. Error messages name the field, never the value
//! of an environment variable or label, which can be a secret.

use super::{Mapping, Value};
use anyhow::{anyhow, bail, Result};
use std::collections::BTreeMap;

/// Text of a scalar. Strings pass through, booleans become `true` or `false`,
/// integers become decimal text. A fractional number is refused: quote it in
/// the compose file so its digits are kept.
pub(super) fn scalar_text(value: &Value, what: &str) -> Result<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Bool(flag) => Ok(flag.to_string()),
        Value::Number(n) if n.is_i64() || n.is_u64() => Ok(n.to_string()),
        Value::Number(_) => bail!(
            "{what} is a decimal number, which YAML may have altered (3.10 reads as 3.1); quote it"
        ),
        _ => bail!("{what} must be a string, number or boolean"),
    }
}

/// Refuse a line break or NUL byte in text bound for an engine argument.
pub(super) fn check_text(text: &str, what: &str) -> Result<()> {
    if text.contains(['\n', '\r', '\0']) {
        bail!("{what} contains a line break or NUL byte");
    }
    Ok(())
}

/// [`scalar_text`] followed by [`check_text`].
pub(super) fn text_value(value: &Value, what: &str) -> Result<String> {
    let text = match scalar_text(value, what) {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    match check_text(&text, what) {
        Ok(()) => Ok(text),
        Err(e) => Err(e),
    }
}

/// A list of scalars, such as an exec form `command`.
pub(super) fn string_list(value: &Value, what: &str) -> Result<Vec<String>> {
    let Value::Array(items) = value else {
        bail!("{what} must be a list");
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match text_value(item, what) {
            Ok(text) => out.push(text),
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// Check a key that becomes `KEY=value` in an engine argument.
pub(super) fn check_pair_key(key: &str, what: &str) -> Result<()> {
    if key.is_empty() || key.contains(|c: char| c == '=' || c.is_control() || c.is_whitespace()) {
        bail!("invalid {what} name {key:?}");
    }
    Ok(())
}

/// A map of text, written either as a mapping or as a list of `key=value`.
/// A null value in a mapping is the empty string. Used for labels and build
/// args.
pub(super) fn string_map(value: &Value, what: &str) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    match value {
        Value::Object(entries) => {
            for (key, raw) in entries {
                let text = match raw {
                    Value::Null => String::new(),
                    other => match text_value(other, &format!("{what} '{key}'")) {
                        Ok(text) => text,
                        Err(e) => return Err(e),
                    },
                };
                out.insert(key.clone(), text);
            }
        }
        Value::Array(items) => {
            for item in items {
                let Value::String(entry) = item else {
                    bail!("{what} list entries must be strings like key=value");
                };
                let Some((key, text)) = entry.split_once('=') else {
                    bail!("{what} entry {entry:?} must look like key=value");
                };
                if let Err(e) = check_text(text, &format!("{what} '{key}'")) {
                    return Err(e);
                }
                out.insert(key.to_string(), text.to_string());
            }
        }
        _ => bail!("{what}s must be a mapping or a list of key=value"),
    }
    for key in out.keys() {
        if let Err(e) = check_pair_key(key, what) {
            return Err(e);
        }
    }
    Ok(out)
}

/// Refuse keys outside `allowed`, naming the key.
pub(super) fn reject_unknown(map: &Mapping, allowed: &[&str], what: &str) -> Result<()> {
    match map.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(anyhow!("{what}: unsupported key '{key}'")),
        None => Ok(()),
    }
}
