//! Variable interpolation for compose scalar values.
//!
//! Supports `$VAR`, `${VAR}`, `${VAR:-default}` and `$$`, as docker compose
//! does. Every other `${...}` form, and a `$` that starts none of these, is an
//! error naming the expression, so a typo never turns into a silent empty
//! string. Expansion is a single pass: a substituted value is never scanned
//! again, so a variable cannot smuggle in a further expansion.

use anyhow::{anyhow, bail, Result};
use std::collections::BTreeMap;

/// The expanded text would pass the byte cap given to [`interpolate_capped`].
/// Callers tell it apart from a syntax error with `downcast_ref`.
#[derive(Debug)]
pub(crate) struct TooLarge;

impl std::fmt::Display for TooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the expanded value is too large")
    }
}

impl std::error::Error for TooLarge {}

/// Expand every variable reference in `text`.
///
/// An unset variable expands to an empty string and adds one warning per
/// variable name to `warnings`.
pub(crate) fn interpolate(
    text: &str,
    vars: &BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) -> Result<String> {
    let lookup = |name: &str| vars.get(name).map(String::as_str);
    interpolate_capped(text, &lookup, usize::MAX, warnings)
}

/// [`interpolate`] with variables from `lookup`, failing with [`TooLarge`]
/// as soon as the output would pass `cap` bytes, before it is allocated.
pub(crate) fn interpolate_capped<'t, 'v>(
    text: &'t str,
    lookup: &dyn Fn(&str) -> Option<&'v str>,
    cap: usize,
    warnings: &mut Vec<String>,
) -> Result<String> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        if let Err(e) = push_capped(&mut out, &rest[..at], cap) {
            return Err(e);
        }
        let expression = match expression_at(&rest[at..]) {
            Ok(expression) => expression,
            Err(e) => return Err(e),
        };
        let value = match expand(expression, lookup, warnings) {
            Ok(value) => value,
            Err(e) => return Err(e),
        };
        if let Err(e) = push_capped(&mut out, value, cap) {
            return Err(e);
        }
        rest = &rest[at + expression.len()..];
    }
    match push_capped(&mut out, rest, cap) {
        Ok(()) => Ok(out),
        Err(e) => Err(e),
    }
}

fn push_capped(out: &mut String, piece: &str, cap: usize) -> Result<()> {
    if out.len().saturating_add(piece.len()) > cap {
        return Err(anyhow::Error::new(TooLarge));
    }
    out.push_str(piece);
    Ok(())
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit())
}

/// The reference at the start of `s`, which begins with `$`.
fn expression_at(s: &str) -> Result<&str> {
    let after = &s[1..];
    if after.starts_with('$') {
        return Ok("$$");
    }
    if after.starts_with('{') {
        return braced_expression(s);
    }
    let name_len = after.find(|c| !is_name_char(c)).unwrap_or(after.len());
    if !is_valid_name(&after[..name_len]) {
        let shown: String = s.chars().take(2).collect();
        bail!("unsupported interpolation '{shown}'; write $$ for a literal dollar sign");
    }
    Ok(&s[..1 + name_len])
}

/// `${...}` up to its matching brace, so a nested `${A:-${B}}` is reported whole.
fn braced_expression(s: &str) -> Result<&str> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&s[..=i]);
                }
            }
            _ => {}
        }
    }
    Err(anyhow!("unterminated interpolation '{s}'"))
}

fn expand<'x, 't: 'x, 'v: 'x>(
    expression: &'t str,
    lookup: &dyn Fn(&str) -> Option<&'v str>,
    warnings: &mut Vec<String>,
) -> Result<&'x str> {
    if expression == "$$" {
        return Ok("$");
    }
    let Some(body) = expression
        .strip_prefix("${")
        .and_then(|b| b.strip_suffix('}'))
    else {
        return Ok(value_or_warn(&expression[1..], lookup, warnings));
    };

    let name_len = body.find(|c| !is_name_char(c)).unwrap_or(body.len());
    let (name, tail) = body.split_at(name_len);
    if !is_valid_name(name) {
        bail!("unsupported interpolation '{expression}'");
    }
    if tail.is_empty() {
        return Ok(value_or_warn(name, lookup, warnings));
    }
    match tail.strip_prefix(":-") {
        Some(default) if !default.contains('$') => {
            Ok(lookup(name).filter(|value| !value.is_empty()).unwrap_or(default))
        }
        _ => bail!("unsupported interpolation '{expression}'; only ${{VAR}} and ${{VAR:-default}} are supported"),
    }
}

fn value_or_warn<'v>(
    name: &str,
    lookup: &dyn Fn(&str) -> Option<&'v str>,
    warnings: &mut Vec<String>,
) -> &'v str {
    if let Some(value) = lookup(name) {
        return value;
    }
    let warning = format!("variable {name} is not set; using an empty string");
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
    ""
}

#[cfg(test)]
#[path = "interpolate_tests.rs"]
mod tests;
