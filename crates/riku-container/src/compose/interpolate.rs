//! Variable interpolation for compose scalar values.
//!
//! Supports `$VAR`, `${VAR}`, `${VAR:-default}` and `$$`, as docker compose
//! does. Every other `${...}` form, and a `$` that starts none of these, is an
//! error naming the expression, so a typo never turns into a silent empty
//! string. Expansion is a single pass: a substituted value is never scanned
//! again, so a variable cannot smuggle in a further expansion.

use anyhow::{anyhow, bail, Result};
use std::collections::BTreeMap;

/// Expand every variable reference in `text`.
///
/// An unset variable expands to an empty string and adds one warning per
/// variable name to `warnings`.
pub(crate) fn interpolate(
    text: &str,
    vars: &BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let expression = match expression_at(&rest[at..]) {
            Ok(expression) => expression,
            Err(e) => return Err(e),
        };
        match expand(expression, vars, warnings) {
            Ok(value) => out.push_str(&value),
            Err(e) => return Err(e),
        }
        rest = &rest[at + expression.len()..];
    }
    out.push_str(rest);
    Ok(out)
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

fn expand(
    expression: &str,
    vars: &BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) -> Result<String> {
    if expression == "$$" {
        return Ok("$".to_string());
    }
    let Some(body) = expression
        .strip_prefix("${")
        .and_then(|b| b.strip_suffix('}'))
    else {
        return Ok(lookup(&expression[1..], vars, warnings));
    };

    let name_len = body.find(|c| !is_name_char(c)).unwrap_or(body.len());
    let (name, tail) = body.split_at(name_len);
    if !is_valid_name(name) {
        bail!("unsupported interpolation '{expression}'");
    }
    if tail.is_empty() {
        return Ok(lookup(name, vars, warnings));
    }
    match tail.strip_prefix(":-") {
        Some(default) if !default.contains('$') => Ok(vars
            .get(name)
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| default.to_string())),
        _ => bail!("unsupported interpolation '{expression}'; only ${{VAR}} and ${{VAR:-default}} are supported"),
    }
}

fn lookup(name: &str, vars: &BTreeMap<String, String>, warnings: &mut Vec<String>) -> String {
    if let Some(value) = vars.get(name) {
        return value.clone();
    }
    let warning = format!("variable {name} is not set; using an empty string");
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
    String::new()
}

#[cfg(test)]
#[path = "interpolate_tests.rs"]
mod tests;
