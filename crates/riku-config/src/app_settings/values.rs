//! Typed `riku.toml` values: checking a TOML value against its key, and
//! rendering it into the env map.
//!
//! Security: a repo file is pushed input. Its text and list items may only
//! hold letters, digits and `. - _ / : @ + *`, so nothing that reaches the
//! router config, an env file or a shell can carry quoting, separators or
//! control characters. `document_root` must be a relative path with no `..`.
//! The server override is operator input, so it is only refused control
//! characters (they would corrupt an env file). Error messages name the key,
//! never the value, because a misspelled secret key would otherwise print the
//! secret.

use super::keys::{Key, Kind};
use super::Source;

const REPO_EXTRA_CHARS: &[u8] = b".-_/:@+*";
const ROUTING_MODES: &[&str] = &["domain", "port", "path"];

/// One checked value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Int(u64),
    List(Vec<String>),
    Bool(bool),
}

fn repo_safe(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || REPO_EXTRA_CHARS.contains(&b))
}

fn check_text(key: &Key, text: &str, source: Source) -> Result<(), String> {
    let path = key.path();
    if text.chars().any(char::is_control) {
        return Err(format!("{path} contains a control character"));
    }
    if source == Source::Repo && !repo_safe(text) {
        return Err(format!(
            "{path} may only contain letters, digits and . - _ / : @ + *"
        ));
    }
    if path == "routing.mode" && !ROUTING_MODES.contains(&text) {
        return Err(format!("{path} must be \"domain\", \"port\" or \"path\""));
    }
    if path == "routing.document_root"
        && (text.starts_with('/') || text.split('/').any(|part| part == ".."))
    {
        return Err(format!("{path} must be a relative path inside the app"));
    }
    Ok(())
}

fn wrong_type(key: &Key, expected: &str) -> String {
    format!("{} must be {expected}", key.path())
}

fn check_list(key: &Key, value: &toml::Value, source: Source) -> Result<Value, String> {
    let items = match value.as_array() {
        Some(items) => items,
        None => return Err(wrong_type(key, "an array of strings")),
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let text = match item.as_str() {
            Some(text) => text,
            None => return Err(wrong_type(key, "an array of strings")),
        };
        if text.contains(',') {
            return Err(format!("{} items may not contain a comma", key.path()));
        }
        if let Err(e) = check_text(key, text, source) {
            return Err(e);
        }
        out.push(text.to_string());
    }
    Ok(Value::List(out))
}

/// Check a TOML value against `key`.
pub fn check(key: &Key, value: &toml::Value, source: Source) -> Result<Value, String> {
    match key.kind {
        Kind::Text => match value.as_str() {
            Some(text) => match check_text(key, text, source) {
                Ok(()) => Ok(Value::Text(text.to_string())),
                Err(e) => Err(e),
            },
            None => Err(wrong_type(key, "a string")),
        },
        Kind::Int if key.path() == "routing.port" => match value.as_integer() {
            Some(n) if (1..=i64::from(u16::MAX)).contains(&n) => Ok(Value::Int(n as u64)),
            _ => Err(wrong_type(key, "a port number from 1 to 65535")),
        },
        Kind::Int => match value.as_integer() {
            Some(n) if n >= 0 => Ok(Value::Int(n as u64)),
            _ => Err(wrong_type(key, "a whole number, 0 or more")),
        },
        Kind::Flag | Kind::Bool => match value.as_bool() {
            Some(b) => Ok(Value::Bool(b)),
            None => Err(wrong_type(key, "true or false")),
        },
        Kind::List => check_list(key, value, source),
    }
}

/// The env value for `value`, or `None` when the key is left out (a false
/// `Flag`).
pub fn render(key: &Key, value: &Value) -> Option<String> {
    match (key.kind, value) {
        (Kind::Flag, Value::Bool(false)) => None,
        (_, Value::Bool(b)) => Some(b.to_string()),
        (_, Value::Int(n)) => Some(n.to_string()),
        (_, Value::List(items)) => Some(items.join(",")),
        (_, Value::Text(text)) => Some(text.clone()),
    }
}

/// The TOML form of `value`.
pub fn to_toml(value: &Value) -> toml::Value {
    match value {
        Value::Text(text) => toml::Value::String(text.clone()),
        Value::Int(n) => toml::Value::Integer(*n as i64),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::List(items) => {
            toml::Value::Array(items.iter().cloned().map(toml::Value::String).collect())
        }
    }
}

/// `text` (from the command line or an env file) as a TOML value of `key`'s
/// kind, for [`check`]. `None` when it cannot be that kind.
pub fn parse_text(key: &Key, text: &str) -> Option<toml::Value> {
    match key.kind {
        Kind::Text => Some(toml::Value::String(text.to_string())),
        Kind::Int => match text.trim().parse::<i64>() {
            Ok(n) => Some(toml::Value::Integer(n)),
            Err(_) => None,
        },
        Kind::Flag | Kind::Bool => match text.trim() {
            "true" => Some(toml::Value::Boolean(true)),
            "false" => Some(toml::Value::Boolean(false)),
            _ => None,
        },
        Kind::List => Some(toml::Value::Array(
            text.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| toml::Value::String(item.to_string()))
                .collect(),
        )),
    }
}
