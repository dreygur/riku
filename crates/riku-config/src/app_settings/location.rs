//! Turning a parse problem into an error that names file, line and key.

use std::path::Path;

use anyhow::anyhow;

use super::Problem;

fn header_name(line: &str) -> Option<&str> {
    let line = line.trim();
    match line
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        Some(name) => Some(name.trim()),
        None => None,
    }
}

fn assigns(line: &str, key: &str) -> bool {
    match line.trim_start().strip_prefix(key) {
        Some(rest) => rest.trim_start().starts_with('='),
        None => false,
    }
}

/// The 1-based line where `section.key` is assigned. Only the plain
/// `[section]` then `key = ...` layout is found; anything else gives `None`
/// and the error just names the file.
fn key_line(raw: &str, section: &str, key: &str) -> Option<usize> {
    let mut current = "";
    for (index, line) in raw.lines().enumerate() {
        if let Some(name) = header_name(line) {
            current = name;
            continue;
        }
        if current == section && assigns(line, key) {
            return Some(index + 1);
        }
    }
    None
}

fn line_of_offset(raw: &str, offset: usize) -> usize {
    raw[..offset.min(raw.len())].matches('\n').count() + 1
}

/// `problem` as an error naming the file and, when found, the line.
pub fn located(raw: &str, file: &Path, problem: Problem) -> anyhow::Error {
    let line = match &problem.key {
        Some(key) => key_line(raw, &problem.section, key),
        None => None,
    };
    match line {
        Some(line) => anyhow!("{}:{line}: {}", file.display(), problem.message),
        None => anyhow!("{}: {}", file.display(), problem.message),
    }
}

/// A TOML syntax error, naming the line from its span. The message is the
/// parser's own, without the source excerpt that would echo a value.
pub fn syntax(raw: &str, file: &Path, error: &toml::de::Error) -> anyhow::Error {
    match error.span() {
        Some(span) => anyhow!(
            "{}:{}: invalid TOML: {}",
            file.display(),
            line_of_offset(raw, span.start),
            error.message()
        ),
        None => anyhow!("{}: invalid TOML: {}", file.display(), error.message()),
    }
}
