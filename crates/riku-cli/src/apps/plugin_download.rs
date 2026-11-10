//! Downloading the java and clojure runtime plugins, the only plugins riku
//! does not carry.
//!
//! Security: a binary comes from the release tag of this riku's own version,
//! never `latest`, and is installed only when its sha256 matches the
//! `.sha256` file published with it. That catches corruption and a swapped
//! asset, not a compromised release (the hash comes from the same release).

use std::io::Read;

use anyhow::{anyhow, bail, Result};
use sha2::{Digest, Sha256};

use crate::config::RikuPaths;
use crate::util::display;

use super::plugin_write::write_plugin_file;

const RELEASE_BASE: &str = "https://github.com/dreygur/riku/releases/download";
const EXECUTABLE: u32 = 0o755;
const SHA256_HEX_LEN: usize = 64;
/// Far above any plugin binary riku publishes.
const MAX_BINARY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SUMS_BYTES: u64 = 4096;
/// The triple this riku was built for, set by build.rs.
const BUILD_TARGET: &str = env!("RIKU_TARGET");
/// Every triple release.yml publishes plugins for. Keep in step with its
/// build matrix.
const RELEASE_TRIPLES: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-gnu",
    "aarch64-unknown-linux-musl",
    "armv7-unknown-linux-gnueabihf",
    "armv7-unknown-linux-musleabihf",
    "arm-unknown-linux-gnueabihf",
    "arm-unknown-linux-musleabihf",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
];

/// Why a download failed.
enum FetchError {
    NotFound,
    Failed(String),
}

fn fetch(url: &str, limit: u64) -> Result<Vec<u8>, FetchError> {
    let client = match riku_http::blocking_client_builder().build() {
        Ok(client) => client,
        Err(e) => {
            return Err(FetchError::Failed(format!(
                "cannot build the download client: {e}"
            )))
        }
    };
    let response = match client.get(url).send() {
        Ok(response) => response,
        Err(e) => return Err(FetchError::Failed(format!("cannot fetch {url}: {e}"))),
    };
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(FetchError::NotFound);
    }
    if !status.is_success() {
        return Err(FetchError::Failed(format!(
            "HTTP {status} when fetching {url}"
        )));
    }
    read_capped(response, limit, url)
}

/// All of `body`, refusing more than `limit` bytes so a wrong or hostile
/// response cannot exhaust memory.
fn read_capped(body: impl Read, limit: u64, url: &str) -> Result<Vec<u8>, FetchError> {
    let mut bytes = Vec::new();
    if let Err(e) = body.take(limit + 1).read_to_end(&mut bytes) {
        return Err(FetchError::Failed(format!("cannot read {url}: {e}")));
    }
    if bytes.len() as u64 > limit {
        return Err(FetchError::Failed(format!(
            "{url} is more than {limit} bytes; not installed"
        )));
    }
    Ok(bytes)
}

/// The release triple to fetch plugin `name` for, when riku was built for
/// `target`. Matching the build triple, not the host's os and arch, keeps an
/// armv6 or musl riku from fetching an armv7 or glibc plugin it cannot run.
fn release_triple(name: &str, target: &str) -> Result<&'static str> {
    match RELEASE_TRIPLES.iter().find(|triple| **triple == target) {
        Some(triple) => Ok(triple),
        None => bail!(
            "no pre-built '{name}' binary plugin for {target}: see .github/workflows/release.yml for supported targets"
        ),
    }
}

fn binary_url(name: &str, version: &str, triple: &str) -> String {
    format!("{RELEASE_BASE}/v{version}/riku-plugin-{name}-{triple}")
}

/// The hash in `sha256sum` output: the first field, when it is 64 hex digits.
fn expected_sha256(text: &str) -> Option<&str> {
    match text.split_whitespace().next() {
        Some(hash)
            if hash.len() == SHA256_HEX_LEN && hash.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Some(hash)
        }
        _ => None,
    }
}

fn verify(name: &str, version: &str, bytes: &[u8], expected: &str) -> Result<()> {
    let actual = hex::encode(Sha256::digest(bytes));
    if actual.eq_ignore_ascii_case(expected) {
        return Ok(());
    }
    bail!("checksum mismatch for plugin '{name}' (release v{version}); not installed")
}

fn fetch_or_explain(
    url: &str,
    limit: u64,
    name: &str,
    version: &str,
    triple: &str,
) -> Result<Vec<u8>> {
    match fetch(url, limit) {
        Ok(bytes) => Ok(bytes),
        Err(FetchError::NotFound) => Err(anyhow!(
            "no release v{version} of plugin '{name}' for {triple}"
        )),
        Err(FetchError::Failed(reason)) => Err(anyhow!(reason)),
    }
}

/// Download `name` (java or clojure) for this host from this riku's release,
/// check it, and install it as `<plugin_root>/<name>`.
#[allow(clippy::question_mark)]
pub(crate) fn install_binary_plugin(paths: &RikuPaths, name: &str) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let triple = match release_triple(name, BUILD_TARGET) {
        Ok(triple) => triple,
        Err(e) => return Err(e),
    };
    let url = binary_url(name, version, triple);
    display::info(&format!("Downloading {name} from {url}..."));
    let bytes = match fetch_or_explain(&url, MAX_BINARY_BYTES, name, version, triple) {
        Ok(bytes) => bytes,
        Err(e) => return Err(e),
    };
    let sums_url = format!("{url}.sha256");
    let sums = match fetch_or_explain(&sums_url, MAX_SUMS_BYTES, name, version, triple) {
        Ok(sums) => sums,
        Err(e) => return Err(e),
    };
    let expected = match expected_sha256(&String::from_utf8_lossy(&sums)) {
        Some(hash) => hash.to_string(),
        None => {
            return Err(anyhow!(
                "the checksum file of plugin '{name}' is not sha256sum output"
            ))
        }
    };
    if let Err(e) = verify(name, version, &bytes, &expected) {
        return Err(e);
    }
    write_plugin_file(&paths.plugin_root, name, &bytes, EXECUTABLE)
}

#[cfg(test)]
#[path = "plugin_download_tests.rs"]
mod tests;
