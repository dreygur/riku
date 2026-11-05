//! The one rule for text placed inside a double quoted nginx string.
//!
//! The route gate and the nginx renderer both apply it, so a value the gate
//! lets through is never one the renderer would write differently.

/// Characters refused inside a double quoted value. nginx ends the string at
/// `"`, reads `$` as a variable and `\` as an escape. `;`, `{`, `}`, `'` and
/// `#` are plain text there.
const QUOTED_REFUSED: [char; 3] = ['"', '$', '\\'];

/// Whether `value` holds a control character or one of `" $ \`.
pub fn breaks_quoted_string(value: &str) -> bool {
    value.contains(|c: char| c.is_control() || QUOTED_REFUSED.contains(&c))
}
