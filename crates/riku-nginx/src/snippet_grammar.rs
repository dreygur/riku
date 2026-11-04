//! The grammar an app's nginx snippet must fit, and the split into
//! statements that the directive allowlist is checked against.
//!
//! Security model: the snippet comes from the app's repo and is pasted into
//! a server block of the one server-wide nginx, so riku must see exactly the
//! statements nginx will see. Rather than reproduce nginx's tokenizer, the
//! accepted language is a strict subset of it where the two cannot differ:
//!
//! - Only printable ASCII, space, tab and newline. No other control
//!   character, no CR, no non-ASCII.
//! - No backslash anywhere. nginx escapes with it (inside words and quotes,
//!   even before a space or newline), and no allowlisted directive needs it.
//! - No `{` or `}` anywhere, comments and quotes included: they would close
//!   the app's server block and open another.
//! - A word is a run of characters other than space, tab, newline and `;`.
//!   A `"`, `'` or `#` is special only at the start of a word, as in nginx,
//!   and refused anywhere inside one, where nginx would read it literally.
//! - `#` at a word start begins a comment that runs to the end of the line.
//! - A quoted string ends at the same quote character, must not span lines,
//!   must be closed, and must be followed by a space, tab, newline or `;`.
//! - `;` outside a quoted string ends a statement. Every statement must be
//!   ended by one, and none may be empty (`;;` or a leading `;`).
//!
//! Anything outside this grammar is refused with the reason, never repaired.

/// Why `c` may not appear in a snippet at all, if it may not.
fn refused_character(c: char) -> Option<&'static str> {
    match c {
        '\\' => Some("a backslash"),
        '{' | '}' => Some("a { or }"),
        ' ' | '\t' | '\n' | '!'..='~' => None,
        _ => Some("a character other than printable ASCII, space, tab or newline"),
    }
}

/// The snippet's statements, split as nginx splits them, or why it is
/// refused. Each statement keeps its words and quotes; comments are dropped.
pub(super) fn statements(content: &str) -> Result<Vec<String>, String> {
    if let Some((reason, c)) = content
        .chars()
        .find_map(|c| refused_character(c).map(|reason| (reason, c)))
    {
        return Err(format!("contains {reason} ({c:?})"));
    }

    let mut statements = vec![String::new()];
    let mut quote: Option<char> = None;
    let mut in_comment = false;
    let mut in_word = false;
    let mut need_space = false;
    for c in content.chars() {
        if in_comment {
            in_comment = c != '\n';
            continue;
        }
        let current = statements.last_mut().expect("starts with one statement");
        if let Some(open) = quote {
            if c == '\n' {
                return Err("has a quoted string that runs past the end of its line".to_string());
            }
            current.push(c);
            if c == open {
                quote = None;
                need_space = true;
            }
            continue;
        }
        let ends_word = matches!(c, ' ' | '\t' | '\n' | ';');
        if need_space && !ends_word {
            return Err("has a closing quote not followed by a space or ;".to_string());
        }
        need_space = false;
        match c {
            '"' | '\'' | '#' if in_word => {
                return Err(format!(
                    "has a {c} inside a word, which nginx reads differently"
                ));
            }
            '"' | '\'' => {
                quote = Some(c);
                current.push(c);
            }
            '#' => in_comment = true,
            ';' if current.trim().is_empty() => {
                return Err("has an empty statement (a ; with nothing before it)".to_string());
            }
            ';' => {
                statements.push(String::new());
                in_word = false;
            }
            ' ' | '\t' | '\n' => {
                current.push(c);
                in_word = false;
            }
            _ => {
                current.push(c);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return Err("has a quote that is never closed".to_string());
    }
    match statements.pop() {
        Some(tail) if !tail.trim().is_empty() => Err(format!(
            "has a statement with no closing ; ({:?})",
            tail.trim()
        )),
        _ => Ok(statements),
    }
}
