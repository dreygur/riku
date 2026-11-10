//! Downloading the java and clojure runtime plugins, the only plugins riku
//! does not carry.
//!
//! Security: a binary comes from the release tag of this riku's own version,
//! never `latest`, and is installed only when its sha256 matches the
//! `.sha256` file published with it. That catches corruption and a swapped
//! asset, not a compromised release (the hash comes from the same release).

use anyhow::{anyhow, bail, Result};
use sha2::{Digest, Sha256};

use crate::config::RikuPaths;
use crate::util::display;

use super::plugin_write::write_plugin_file;

const RELEASE_BASE: &str = "https://github.com/dreygur/riku/releases/download";
const EXECUTABLE: u32 = 0o755;
const SHA256_HEX_LEN: usize = 64;

/// Why a download failed.
enum FetchError {
    NotFound,
    Failed(String),
}

fn fetch(url: &str) -> Result<Vec<u8>, FetchError> {
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
    match response.bytes() {
        Ok(body) => Ok(body.to_vec()),
        Err(e) => Err(FetchError::Failed(format!("cannot read {url}: {e}"))),
    }
}

/// The release target triple riku builds plugins for on `os`/`arch`.
fn target_triple_for(name: &str, os: &str, arch: &str) -> Result<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        ("linux", "arm") => Ok("armv7-unknown-linux-gnueabihf"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        (os, arch) => bail!(
            "no pre-built '{name}' binary plugin for {os}/{arch}: see .github/workflows/release.yml for supported targets"
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

fn fetch_or_explain(url: &str, name: &str, version: &str, triple: &str) -> Result<Vec<u8>> {
    match fetch(url) {
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
    let triple = match target_triple_for(name, std::env::consts::OS, std::env::consts::ARCH) {
        Ok(triple) => triple,
        Err(e) => return Err(e),
    };
    let url = binary_url(name, version, triple);
    display::info(&format!("Downloading {name} from {url}..."));
    let bytes = match fetch_or_explain(&url, name, version, triple) {
        Ok(bytes) => bytes,
        Err(e) => return Err(e),
    };
    let sums = match fetch_or_explain(&format!("{url}.sha256"), name, version, triple) {
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
