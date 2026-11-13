//! The notice docker prints on every build when the buildx plugin is
//! missing: a `DEPRECATED: The legacy builder is deprecated` line, a few
//! indented lines saying how to install buildx, and a blank line. It says
//! nothing about the app being built, so it is kept off the deploy rail;
//! `riku doctor` warns about the missing plugin instead.

const FIRST_LINE: &str = "DEPRECATED: The legacy builder is deprecated";

/// Tracks whether one output stream is inside the notice.
#[derive(Default)]
pub(super) struct LegacyNotice {
    inside: bool,
}

impl LegacyNotice {
    /// Whether `line` is part of the notice. The notice is its first line,
    /// the indented lines after it, and the blank line that ends it; any
    /// other line ends it and is kept.
    pub(super) fn swallows(&mut self, line: &str) -> bool {
        if line.starts_with(FIRST_LINE) {
            self.inside = true;
            return true;
        }
        if !self.inside {
            return false;
        }
        if line.starts_with(' ') {
            return true;
        }
        self.inside = false;
        line.is_empty()
    }
}
