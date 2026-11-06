//! The catch-all server root init adds to riku's include file when nothing
//! else on the host is the default for port 80. Without one, nginx answers
//! an unknown Host with whichever riku site it loaded first.

use std::path::Path;

/// Closes the connection for any Host no site claims.
pub const CATCH_ALL_SERVER: &str =
    "server {\n  listen 80 default_server;\n  server_name _;\n  return 444;\n}\n";

const FILE_HEADER: &str = "# configuration file ";

/// Whether riku should add [`CATCH_ALL_SERVER`]. `dump` is the output of
/// `nginx -T`; `own_file` is riku's include file, which is skipped so a
/// catch-all riku wrote before does not count as someone else's. nginx
/// refuses two `default_server` on one address and port, so any other
/// default on port 80 means no.
pub fn needs_catch_all(dump: &str, own_file: &Path) -> bool {
    let own_header = format!("{FILE_HEADER}{}:", own_file.display());
    let mut in_own_file = false;
    for line in dump.lines() {
        if line.starts_with(FILE_HEADER) {
            in_own_file = line.trim_end() == own_header;
            continue;
        }
        if !in_own_file && is_default_on_port_80(line) {
            return false;
        }
    }
    true
}

fn is_default_on_port_80(line: &str) -> bool {
    let directive = line.split('#').next().unwrap_or_default();
    let mut words = directive
        .split(|c: char| c.is_whitespace() || c == ';')
        .filter(|word| !word.is_empty());
    if words.next() != Some("listen") {
        return false;
    }
    let on_port_80 = match words.next() {
        Some(address) => address == "80" || address.ends_with(":80"),
        None => false,
    };
    on_port_80 && words.any(|word| word == "default_server" || word == "default")
}

#[cfg(test)]
#[path = "default_server_tests.rs"]
mod tests;
