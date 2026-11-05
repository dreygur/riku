//! Checks on the free text fields of a route: locations, the catch all file
//! name and header text. Each one ends up inside a directive of a config the
//! router parses as root.

use anyhow::{anyhow, Result};
use riku_nginx::breaks_directive;

/// Characters refused inside a double quoted value. nginx ends the string at
/// `"`, reads `$` as a variable and `\` as an escape. `;`, `{`, `}`, `'` and
/// `#` are plain text there.
const QUOTED_REFUSED: [char; 3] = ['"', '$', '\\'];

fn plain(c: char) -> bool {
    !breaks_directive(c) && !matches!(c, '\\' | '`')
}

/// A URL location prefix such as `/static/`: it starts with `/` and holds
/// nothing that could end the directive it is written into.
pub(crate) fn location(field: &str, value: &str) -> Result<()> {
    if !value.starts_with('/') || !value.chars().all(plain) {
        return Err(anyhow!(
            "{field} {value:?} must start with / and hold no whitespace, quotes or nginx syntax characters"
        ));
    }
    Ok(())
}

/// A file name relative to the site root: letters, digits, `.`, `_`, `-` and
/// `/`, no leading `/` and no `..` component.
pub(crate) fn relative_file(field: &str, value: &str) -> Result<()> {
    let name_ok = value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'));
    let escapes = value.split('/').any(|part| part == "..");
    if value.is_empty() || value.starts_with('/') || !name_ok || escapes {
        return Err(anyhow!(
            "{field} {value:?} must be a relative file name of letters, digits, . _ - and /"
        ));
    }
    Ok(())
}

/// Text written inside double quotes, such as a `Content-Security-Policy`
/// value: it must not close the string, interpolate or contain a line break.
pub(crate) fn header_text(field: &str, value: &str) -> Result<()> {
    if value.contains(|c: char| c.is_control() || QUOTED_REFUSED.contains(&c)) {
        return Err(anyhow!(
            "{field} {value:?} must not contain control characters or any of \" $ \\"
        ));
    }
    Ok(())
}
