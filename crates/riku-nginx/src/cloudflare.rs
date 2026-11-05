//! The Cloudflare address list a site with the Cloudflare ACL includes.
//!
//! Security: only root init fetches the ranges, and it writes the result to a
//! root owned file. The router helper never fetches and never includes a file
//! the deploy user can write. A response is data: each line must be a bare
//! address or CIDR before it becomes a directive, so a hostile response
//! cannot add nginx syntax to the file.

use std::time::Duration;

use anyhow::{anyhow, Result};

const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const RANGE_LISTS: [(&str, &str); 2] = [
    ("IPv4", "https://www.cloudflare.com/ips-v4"),
    ("IPv6", "https://www.cloudflare.com/ips-v6"),
];

/// Download `url` and return its body. This is the fetch root init passes to
/// [`cloudflare_allow_config`]; tests pass their own.
pub fn fetch_cloudflare_url(url: &str) -> Result<String> {
    let client = match riku_http::blocking_client_builder()
        .timeout(FETCH_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(e) => return Err(anyhow!("cannot build the Cloudflare client: {e}")),
    };
    let response = match client.get(url).send() {
        Ok(response) => response,
        Err(e) => return Err(anyhow!("cannot fetch {url}: {e}")),
    };
    if !response.status().is_success() {
        return Err(anyhow!("{url} answered {}", response.status()));
    }
    match response.text() {
        Ok(body) => Ok(body),
        Err(e) => Err(anyhow!("cannot read {url}: {e}")),
    }
}

/// Only characters of a valid IP or CIDR, so an upstream response cannot
/// inject nginx directives.
fn is_valid_cidr(line: &str) -> bool {
    let valid_chars = line
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == '.' || c == ':' || c == '/');
    valid_chars && (line.contains('.') || line.contains(':'))
}

fn allow_lines(label: &str, body: &str) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for entry in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if is_valid_cidr(entry) {
            lines.push(format!("allow {entry};"));
        } else {
            tracing::warn!("skipping invalid Cloudflare {label} entry: {entry:?}");
        }
    }
    if lines.is_empty() {
        return Err(anyhow!("the Cloudflare {label} list held no address"));
    }
    Ok(lines)
}

/// The text of `cloudflare.conf`: one `allow` line per Cloudflare range,
/// IPv4 then IPv6. `fetch` returns the body of a URL. Either list failing or
/// coming back empty is an error, so a half list is never written.
pub fn cloudflare_allow_config(fetch: impl Fn(&str) -> Result<String>) -> Result<String> {
    let mut content = String::from("# Cloudflare IP ranges, written by riku init\n\n");
    for (label, url) in RANGE_LISTS {
        let body = match fetch(url) {
            Ok(body) => body,
            Err(e) => return Err(anyhow!("Cloudflare {label} ranges: {e}")),
        };
        let lines = match allow_lines(label, &body) {
            Ok(lines) => lines,
            Err(e) => return Err(anyhow!("Cloudflare {label} ranges: {e}")),
        };
        content.push_str(&lines.join("\n"));
        content.push('\n');
    }
    Ok(content)
}

#[cfg(test)]
#[path = "cloudflare_tests.rs"]
mod tests;
