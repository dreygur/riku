//! Compose style `.env` files.
//!
//! The file sits next to the compose file in the pushed app, so it is
//! untrusted. It must resolve inside the app directory and is size capped.
//! The syntax follows docker compose, not riku's own ENV files: `KEY=value`,
//! an optional `export ` prefix, `#` comments, single quotes (literal) and
//! double quotes (with `\n`, `\t`, `\\`, `\"` escapes). Values are not
//! interpolated. A line that is not one of these is an error naming the file
//! and line, never skipped, so a broken variable cannot silently go missing.

use anyhow::{anyhow, bail, Result};
use riku_util::ensure_path_within;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const MAX_APP_FILE_BYTES: u64 = 1024 * 1024;

/// Read the `.env` file at `path` inside `app_path`. A missing file is an
/// empty map.
pub(crate) fn read_dotenv(app_path: &Path, path: &Path) -> Result<BTreeMap<String, String>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let text = match read_app_file(app_path, path) {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    parse_dotenv(&path.display().to_string(), &text)
}

/// Read a text file from the pushed app: it must resolve inside `app_path`
/// (symlinks included) and be at most 1 MiB.
pub(crate) fn read_app_file(app_path: &Path, path: &Path) -> Result<String> {
    if let Err(e) = ensure_path_within(path, app_path) {
        return Err(anyhow!("{}: {e}", path.display()));
    }
    match fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_APP_FILE_BYTES => {
            bail!("{}: larger than {MAX_APP_FILE_BYTES} bytes", path.display())
        }
        Ok(_) => {}
        Err(e) => bail!("{}: {e}", path.display()),
    }
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) => bail!("{}: {e}", path.display()),
    }
}

/// Parse `.env` text. `source` names the file in error messages.
pub(crate) fn parse_dotenv(source: &str, text: &str) -> Result<BTreeMap<String, String>> {
    let mut vars = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_line(line) {
            Ok((key, value)) => {
                vars.insert(key, value);
            }
            Err(reason) => bail!("{source}:{}: {reason}", index + 1),
        }
    }
    Ok(vars)
}

fn parse_line(line: &str) -> std::result::Result<(String, String), String> {
    let line = line.strip_prefix("export ").unwrap_or(line);
    let Some((key, raw)) = line.split_once('=') else {
        return Err("expected KEY=value".to_string());
    };
    let key = key.trim();
    if !is_valid_key(key) {
        return Err(format!("invalid variable name '{key}'"));
    }
    match parse_value(raw.trim()) {
        Ok(value) => Ok((key.to_string(), value)),
        Err(reason) => Err(format!("{key}: {reason}")),
    }
}

pub(crate) fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn parse_value(raw: &str) -> std::result::Result<String, String> {
    if let Some(quoted) = raw.strip_prefix('\'') {
        return match quoted.split_once('\'') {
            Some((inner, after)) if is_blank_or_comment(after) => Ok(inner.to_string()),
            _ => Err("unterminated single quote".to_string()),
        };
    }
    if let Some(quoted) = raw.strip_prefix('"') {
        return match closing_double_quote(quoted) {
            Some(end) if is_blank_or_comment(&quoted[end + 1..]) => {
                unescape_double_quoted(&quoted[..end])
            }
            _ => Err("unterminated double quote".to_string()),
        };
    }
    let end = raw.find(" #").unwrap_or(raw.len());
    Ok(raw[..end].trim_end().to_string())
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

fn unescape_double_quoted(inner: &str) -> std::result::Result<String, String> {
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

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
