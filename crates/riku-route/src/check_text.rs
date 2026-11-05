//! Check on the free text a route carries: location prefixes. They end up
//! inside a directive of a config the router parses as root.

use anyhow::{anyhow, Result};
use riku_nginx::breaks_directive;

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
