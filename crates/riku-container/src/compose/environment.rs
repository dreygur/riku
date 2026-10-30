//! Compose style `.env` files.
//!
//! The file sits next to the compose file in the pushed app, so it is
//! untrusted. It must resolve inside the app directory and is size capped.
//! The syntax follows docker compose, not riku's own ENV files:
//!
//! - `KEY=value`, with an optional `export ` prefix, and `#` comments;
//! - a bare `KEY` takes its value from the app's riku ENV, and stays unset when
//!   the ENV does not have it;
//! - single quotes are literal, double quotes take `\n`, `\t`, `\\` and `\"`
//!   escapes, and either may run over several lines until the closing quote;
//! - unquoted and double quoted values expand `$VAR`, `${VAR}`,
//!   `${VAR:-default}` and `$$` from earlier lines of the same file, with the
//!   app's ENV winning over them as it does in compose.
//!
//! A line that is none of these, or a quote still open at the end of the file,
//! is an error naming the file and the line the entry starts on, never skipped,
//! so a broken variable cannot silently go missing.
//!
//! A value may hold line breaks. It can be used for interpolation into the
//! compose file, but every value that reaches the engine is checked for line
//! breaks: an environment variable (from `env_file` or `environment`) holding
//! one is refused when the service is parsed, naming the key, and so is a
//! label.

use super::dotenv_value::{parse_value, OpenQuote, ValueError};
use anyhow::{anyhow, bail, Result};
use riku_util::ensure_path_within;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const MAX_APP_FILE_BYTES: u64 = 1024 * 1024;

/// Cap on the parsed values of one file together, after expansion. Without
/// it, lines that each double the previous value (`B=${A}${A}`) turn a 1 MiB
/// file into gigabytes. Same number as the file cap.
const MAX_EXPANDED_BYTES: usize = MAX_APP_FILE_BYTES as usize;

/// Read the `.env` file at `path` inside `app_path`. A missing file is an
/// empty map. `outside` is the app's ENV (see [`parse_dotenv`]).
pub(crate) fn read_dotenv(
    app_path: &Path,
    path: &Path,
    outside: &BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) -> Result<BTreeMap<String, String>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let text = match read_app_file(app_path, path) {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    parse_dotenv(&path.display().to_string(), &text, outside, warnings)
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

/// Parse `.env` text. `source` names the file in error messages. `outside`
/// supplies bare `KEY` lines and wins over earlier lines when a value is
/// expanded. Unset variables in an expansion are noted in `warnings`.
pub(crate) fn parse_dotenv(
    source: &str,
    text: &str,
    outside: &BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) -> Result<BTreeMap<String, String>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut vars = BTreeMap::new();
    let mut total = 0usize;
    let mut next = 0;
    while next < lines.len() {
        let start = next;
        let line = lines[next].trim();
        next += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let statement = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, first)) = statement.split_once('=') else {
            let key = statement.trim();
            if !is_valid_key(key) {
                bail!("{source}:{}: expected KEY=value or KEY", start + 1);
            }
            if let Some(value) = outside.get(key) {
                vars.insert(key.to_string(), value.clone());
            }
            continue;
        };
        let key = key.trim();
        if !is_valid_key(key) {
            bail!("{source}:{}: invalid variable name '{key}'", start + 1);
        }
        let mut raw = first.trim().to_string();
        let mut quote = OpenQuote::after_first_line(&raw);
        while quote != OpenQuote::None && next < lines.len() {
            raw.push('\n');
            raw.push_str(lines[next]);
            quote = quote.after_line(lines[next]);
            next += 1;
        }
        let budget = MAX_EXPANDED_BYTES.saturating_sub(total);
        let parsed = {
            let lookup = |name: &str| {
                outside
                    .get(name)
                    .or_else(|| vars.get(name))
                    .map(String::as_str)
            };
            parse_value(raw.trim_end(), &lookup, budget, warnings)
        };
        match parsed {
            Ok(value) => {
                total = total.saturating_add(value.len());
                vars.insert(key.to_string(), value);
            }
            Err(ValueError::TooLarge) => bail!(
                "{source}:{}: {key}: values expand past {MAX_EXPANDED_BYTES} bytes",
                start + 1
            ),
            Err(ValueError::Malformed(reason)) => bail!("{source}:{}: {key}: {reason}", start + 1),
        }
    }
    Ok(vars)
}

pub(crate) fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
