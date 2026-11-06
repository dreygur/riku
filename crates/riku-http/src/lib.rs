//! Building HTTP clients for the server side crates.
//!
//! reqwest is built with `rustls-no-provider`, so TLS uses ring and the C
//! based aws-lc is never compiled. That leaves rustls without a crypto
//! provider until one is installed, and building any reqwest client (plain
//! http included) panics without one. Every client riku builds comes from
//! [`blocking_client_builder`], which installs ring first.
//!
//! A crate of its own, not part of riku-util, because the `rk` client links
//! riku-util and must not link reqwest.

/// A blocking reqwest client builder, with ring installed as the process
/// wide rustls provider.
pub fn blocking_client_builder() -> reqwest::blocking::ClientBuilder {
    // Err only means a provider is already installed, which is all we need.
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::blocking::Client::builder()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
