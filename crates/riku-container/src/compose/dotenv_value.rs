//! The value half of a `.env` line: quoting, escapes and expansion.
//!
//! Single quoted values are literal. Double quoted values take the `\n`, `\t`,
//! `\\` and `\"` escapes and are then expanded; unquoted values are cut at a
//! ` #` comment and expanded. Expansion uses the compose interpolation in
//! [`super::interpolate`], so `.env` and the compose file accept the same
//! `$VAR`, `${VAR}`, `${VAR:-default}` and `$$` forms.

use super::interpolate::interpolate;
use std::collections::BTreeMap;

/// True while `raw` opens a quote it has not closed yet, so the value goes on
/// to the next line.
pub(super) fn quote_is_open(raw: &str) -> bool {
    if let Some(quoted) = raw.strip_prefix('\'') {
        return !quoted.contains('\'');
    }
    if let Some(quoted) = raw.strip_prefix('"') {
        return closing_double_quote(quoted).is_none();
    }
    false
}

/// The value written as `raw`, expanded against `scope` where its quoting
/// allows. Unset variables become empty strings and are noted in `warnings`.
pub(super) fn parse_value(
    raw: &str,
    scope: &BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) -> Result<String, String> {
    if let Some(quoted) = raw.strip_prefix('\'') {
        return match quoted.split_once('\'') {
            Some((inner, after)) if is_blank_or_comment(after) => Ok(inner.to_string()),
            _ => Err("unterminated single quote".to_string()),
        };
    }
    let text = if let Some(quoted) = raw.strip_prefix('"') {
        match closing_double_quote(quoted) {
            Some(end) if is_blank_or_comment(&quoted[end + 1..]) => {
                match unescape_double_quoted(&quoted[..end]) {
                    Ok(text) => text,
                    Err(reason) => return Err(reason),
                }
            }
            _ => return Err("unterminated double quote".to_string()),
        }
    } else {
        let end = raw.find(" #").unwrap_or(raw.len());
        raw[..end].trim_end().to_string()
    };
    if !text.contains('$') {
        return Ok(text);
    }
    interpolate(&text, scope, warnings).map_err(|e| e.to_string())
}

fn is_blank_or_comment(after_quote: &str) -> bool {
    let rest = after_quote.trim_start();
    rest.is_empty() || (rest.starts_with('#') && after_quote.starts_with(char::is_whitespace))
}

/// Byte index of the first `"` not preceded by a backslash escape.
fn closing_double_quote(quoted: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, c) in quoted.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return Some(index),
            _ => {}
        }
    }
    None
}

fn unescape_double_quoted(inner: &str) -> Result<String, String> {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Err("unescaped double quote inside the value".to_string()),
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => return Err("value ends with a lone backslash".to_string()),
            },
            other => out.push(other),
        }
    }
    Ok(out)
}
