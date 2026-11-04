//! Upper bounds on the values an app's nginx snippet may set.
//!
//! Security model: the allowlist in [`super::app_snippet`] limits which
//! directives a snippet uses, and this module limits how far each may be
//! turned. Every app shares one nginx, so a 100 GB body limit, a day long
//! timeout or thousands of large buffers in one app's server block would let
//! that app's clients tie up memory, disk and connections for all of them.
//! Values are parsed in nginx's own size and time units; any other spelling
//! is refused rather than guessed at, and so is a value above its cap.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

const MAX_BODY_SIZE: u64 = GIB;
const MAX_TIMEOUT_MS: u64 = 3600 * 1000;
const MAX_BUFFER_SIZE: u64 = MIB;
const MAX_BUFFER_COUNT: u64 = 64;
const MAX_BUSY_BUFFERS_SIZE: u64 = 2 * MIB;
const MAX_ADD_HEADERS: usize = 32;
const MAX_HEADER_VALUE_BYTES: usize = 4 * 1024;

/// The statement's words with any surrounding quotes removed. The snippet
/// grammar has already guaranteed every quote is closed and sits at a word
/// start, and that the only whitespace is space, tab and newline.
fn words(statement: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut rest = statement.trim_start();
    while !rest.is_empty() {
        let (word, tail) = match rest.as_bytes()[0] {
            quote @ (b'"' | b'\'') => {
                let body = &rest[1..];
                match body.find(quote as char) {
                    Some(end) => (&body[..end], &body[end + 1..]),
                    None => (body, ""),
                }
            }
            _ => match rest.find([' ', '\t', '\n']) {
                Some(end) => (&rest[..end], &rest[end..]),
                None => (rest, ""),
            },
        };
        words.push(word);
        rest = tail.trim_start();
    }
    words
}

/// `word` split into its leading digits as a number and the unit after them.
fn number_and_unit(word: &str) -> Option<(u64, &str)> {
    let digits = word.len() - word.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    match word[..digits].parse::<u64>() {
        Ok(number) => Some((number, &word[digits..])),
        Err(_) => None,
    }
}

/// An nginx size (`512`, `8k`, `16m`, `1g`, either case) in bytes.
fn parse_size(word: &str) -> Option<u64> {
    let (number, unit) = match number_and_unit(word) {
        Some(parts) => parts,
        None => return None,
    };
    let scale = match unit {
        "" => 1,
        "k" | "K" => KIB,
        "m" | "M" => MIB,
        "g" | "G" => GIB,
        _ => return None,
    };
    number.checked_mul(scale)
}

/// An nginx time (`500ms`, `30`, `30s`, `5m`, `1h`, `1d`) in milliseconds. A
/// bare number is seconds, as in nginx. Compound forms such as `1m30s` are
/// refused.
fn parse_time_ms(word: &str) -> Option<u64> {
    let (number, unit) = match number_and_unit(word) {
        Some(parts) => parts,
        None => return None,
    };
    let scale = match unit {
        "ms" => 1,
        "" | "s" => 1000,
        "m" => 60 * 1000,
        "h" => 3600 * 1000,
        "d" => 24 * 3600 * 1000,
        _ => return None,
    };
    number.checked_mul(scale)
}

fn only_arg<'a>(name: &str, args: &[&'a str]) -> Result<&'a str, String> {
    match args {
        [arg] => Ok(arg),
        _ => Err(format!(
            "sets `{name}` with {} values, expected one",
            args.len()
        )),
    }
}

fn check_size(name: &str, word: &str, cap: u64, cap_text: &str) -> Result<(), String> {
    match parse_size(word) {
        None => Err(format!(
            "sets `{name}` to `{word}`, which is not an nginx size"
        )),
        Some(bytes) if bytes > cap => {
            Err(format!("sets `{name} {word}`, above the {cap_text} cap"))
        }
        Some(_) => Ok(()),
    }
}

fn check_timeout(name: &str, word: &str) -> Result<(), String> {
    match parse_time_ms(word) {
        None => Err(format!(
            "sets `{name}` to `{word}`, which is not an nginx time"
        )),
        Some(ms) if ms > MAX_TIMEOUT_MS => {
            Err(format!("sets `{name} {word}`, above the 3600s cap"))
        }
        Some(_) => Ok(()),
    }
}

/// `client_max_body_size 0` turns the limit off, so it counts as over the cap.
fn check_body_size(word: &str) -> Result<(), String> {
    if parse_size(word) == Some(0) {
        return Err("sets `client_max_body_size 0`, which removes the 1g cap".to_string());
    }
    check_size("client_max_body_size", word, MAX_BODY_SIZE, "1g")
}

fn check_proxy_buffers(args: &[&str]) -> Result<(), String> {
    let [count, size] = args else {
        return Err(format!(
            "sets `proxy_buffers` with {} values, expected a count and a size",
            args.len()
        ));
    };
    match count.parse::<u64>() {
        Ok(n) if (1..=MAX_BUFFER_COUNT).contains(&n) => {}
        _ => {
            return Err(format!(
                "sets `proxy_buffers` count `{count}`, outside 1 to {MAX_BUFFER_COUNT}"
            ))
        }
    }
    check_size("proxy_buffers", size, MAX_BUFFER_SIZE, "1m")
}

fn check_gzip_level(word: &str) -> Result<(), String> {
    match word.parse::<u8>() {
        Ok(level) if (1..=9).contains(&level) => Ok(()),
        _ => Err(format!("sets `gzip_comp_level {word}`, outside 1 to 9")),
    }
}

fn check_header_value(args: &[&str]) -> Result<(), String> {
    match args.get(1) {
        None => Err("has an `add_header` without a value".to_string()),
        Some(value) if value.len() > MAX_HEADER_VALUE_BYTES => Err(format!(
            "sets `add_header {}` to a {} byte value, above the 4 KiB cap",
            args[0],
            value.len()
        )),
        Some(_) => Ok(()),
    }
}

fn check_statement(name: &str, args: &[&str]) -> Result<(), String> {
    match name {
        "client_max_body_size" => only_arg(name, args).and_then(check_body_size),
        "proxy_buffer_size" => {
            only_arg(name, args).and_then(|word| check_size(name, word, MAX_BUFFER_SIZE, "1m"))
        }
        "proxy_busy_buffers_size" => only_arg(name, args)
            .and_then(|word| check_size(name, word, MAX_BUSY_BUFFERS_SIZE, "2m")),
        "proxy_buffers" => check_proxy_buffers(args),
        "gzip_comp_level" => only_arg(name, args).and_then(check_gzip_level),
        "add_header" => check_header_value(args),
        _ if name.ends_with("_timeout") => {
            args.iter().try_for_each(|word| check_timeout(name, word))
        }
        _ => Ok(()),
    }
}

/// Refuse the snippet's statements when a value is above its cap, or when
/// there are more than 32 `add_header` lines, naming the directive.
pub(super) fn check_limits(statements: &[String]) -> Result<(), String> {
    let mut headers = 0;
    for statement in statements {
        let words = words(statement);
        let Some((name, args)) = words.split_first() else {
            continue;
        };
        if *name == "add_header" {
            headers += 1;
        }
        if let Err(reason) = check_statement(name, args) {
            return Err(reason);
        }
    }
    if headers > MAX_ADD_HEADERS {
        return Err(format!(
            "has {headers} `add_header` statements, more than {MAX_ADD_HEADERS}"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "snippet_limits_tests.rs"]
mod tests;
