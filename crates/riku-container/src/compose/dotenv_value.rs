//! The value half of a `.env` line: quoting, escapes and expansion.
//!
//! Single quoted values are literal. Double quoted values take the `\n`, `\t`,
//! `\\` and `\"` escapes and are then expanded; unquoted values are cut at a
//! ` #` comment and expanded. Expansion uses the compose interpolation in
//! [`super::interpolate`], so `.env` and the compose file accept the same
//! `$VAR`, `${VAR}`, `${VAR:-default}` and `$$` forms.

use super::interpolate::{interpolate_capped, TooLarge};

#[cfg(test)]
thread_local! {
    /// Bytes the open quote scan has read on this thread.
    pub(super) static SCANNED_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Whether a value's opening quote is still open, carried from line to line
/// so a value over many lines is scanned once in total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenQuote {
    /// No quote open: the value ends on this line.
    None,
    Single,
    Double,
}

impl OpenQuote {
    /// The state after the first line of a value, `first`.
    pub(super) fn after_first_line(first: &str) -> Self {
        if let Some(quoted) = first.strip_prefix('\'') {
            return OpenQuote::Single.after_line(quoted);
        }
        if let Some(quoted) = first.strip_prefix('"') {
            return OpenQuote::Double.after_line(quoted);
        }
        OpenQuote::None
    }

    /// The state after `line` is appended to a value with this quote open.
    ///
    /// A backslash escape never carries over a line: a `\` at the end of a
    /// line escapes the line break, so the next line starts unescaped.
    pub(super) fn after_line(self, line: &str) -> Self {
        #[cfg(test)]
        SCANNED_BYTES.with(|n| n.set(n.get() + line.len()));
        let closed = match self {
            OpenQuote::None => true,
            OpenQuote::Single => line.contains('\''),
            OpenQuote::Double => closing_double_quote(line).is_some(),
        };
        if closed {
            OpenQuote::None
        } else {
            self
        }
    }
}

/// The value written as `raw`, expanded through `lookup` where its quoting
/// allows. Unset variables become empty strings and are noted in `warnings`.
/// A value longer than `cap` bytes is refused with [`ValueError::TooLarge`],
/// before an expansion past the cap is allocated.
pub(super) fn parse_value<'v>(
    raw: &str,
    lookup: &dyn Fn(&str) -> Option<&'v str>,
    cap: usize,
    warnings: &mut Vec<String>,
) -> Result<String, ValueError> {
    if let Some(quoted) = raw.strip_prefix('\'') {
        return match quoted.split_once('\'') {
            Some((inner, after)) if is_blank_or_comment(after) => {
                within_cap(inner.to_string(), cap)
            }
            _ => Err(ValueError::Malformed(
                "unterminated single quote".to_string(),
            )),
        };
    }
    let text = if let Some(quoted) = raw.strip_prefix('"') {
        match closing_double_quote(quoted) {
            Some(end) if is_blank_or_comment(&quoted[end + 1..]) => {
                match unescape_double_quoted(&quoted[..end]) {
                    Ok(text) => text,
                    Err(reason) => return Err(ValueError::Malformed(reason)),
                }
            }
            _ => {
                return Err(ValueError::Malformed(
                    "unterminated double quote".to_string(),
                ))
            }
        }
    } else {
        let end = raw.find(" #").unwrap_or(raw.len());
        raw[..end].trim_end().to_string()
    };
    if !text.contains('$') {
        return within_cap(text, cap);
    }
    match interpolate_capped(&text, lookup, cap, warnings) {
        Ok(value) => Ok(value),
        Err(e) if e.downcast_ref::<TooLarge>().is_some() => Err(ValueError::TooLarge),
        Err(e) => Err(ValueError::Malformed(e.to_string())),
    }
}

/// Why a value was refused.
#[derive(Debug)]
pub(super) enum ValueError {
    /// Bad quoting, escapes or interpolation syntax.
    Malformed(String),
    /// The value, expanded, would pass the byte cap.
    TooLarge,
}

fn within_cap(value: String, cap: usize) -> Result<String, ValueError> {
    if value.len() > cap {
        return Err(ValueError::TooLarge);
    }
    Ok(value)
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
