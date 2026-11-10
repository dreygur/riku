//! TLS material in a route: one certificate chain and one matching key.
//!
//! The helper writes the certificate and the key to files with different
//! modes, so nothing from the request is written as it came. Each field is
//! parsed section by section, must hold only what its name says, and the
//! caller gets PEM re-encoded from the parsed DER. Text outside the armor and
//! sections of other kinds (`EC PARAMETERS`, `TRUSTED CERTIFICATE`,
//! `ENCRYPTED PRIVATE KEY`) are dropped by the parser and so never reach the
//! output.

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rustls::crypto::ring::sign::any_supported_type;
use rustls::pki_types::pem::{self, PemObject, SectionKind};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::sign::CertifiedKey;

use crate::schema::Tls;

const LINE_WIDTH: usize = 64;

fn sections(pem: &str) -> Result<Vec<(SectionKind, Vec<u8>)>, pem::Error> {
    <(SectionKind, Vec<u8>)>::pem_slice_iter(pem.as_bytes()).collect()
}

fn armor(label: &str, der: &[u8]) -> String {
    let body = STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for line in body.as_bytes().chunks(LINE_WIDTH) {
        out.push_str(&String::from_utf8_lossy(line));
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

fn certificate_chain(pem: &str) -> Result<Vec<CertificateDer<'static>>> {
    let found = match sections(pem) {
        Ok(found) => found,
        Err(e) => return Err(anyhow!("cert_pem is not valid PEM: {e}")),
    };
    if found.is_empty() {
        return Err(anyhow!("cert_pem holds no certificate"));
    }
    found
        .into_iter()
        .map(|(kind, der)| match kind {
            SectionKind::Certificate => Ok(CertificateDer::from(der)),
            _ => Err(anyhow!(
                "cert_pem holds a section that is not a certificate"
            )),
        })
        .collect()
}

fn private_key(pem: &str) -> Result<PrivateKeyDer<'static>> {
    let mut found = match sections(pem) {
        Ok(found) => found,
        Err(e) => return Err(anyhow!("key_pem is not valid PEM: {e}")),
    };
    if found.len() != 1 {
        return Err(anyhow!(
            "key_pem must hold exactly one private key, found {} PEM sections",
            found.len()
        ));
    }
    let (kind, der) = found.remove(0);
    match PrivateKeyDer::from_pem(kind, der) {
        Some(key) => Ok(key),
        None => Err(anyhow!("key_pem does not hold a private key")),
    }
}

fn key_label(key: &PrivateKeyDer<'_>) -> &'static str {
    match key {
        PrivateKeyDer::Pkcs1(_) => "RSA PRIVATE KEY",
        PrivateKeyDer::Sec1(_) => "EC PRIVATE KEY",
        _ => "PRIVATE KEY",
    }
}

/// Refuse a pair that is not a certificate chain plus the key of its first
/// certificate, and return the pair re-encoded from what was parsed.
pub fn check_tls(tls: &Tls) -> Result<Tls> {
    let chain = match certificate_chain(&tls.cert_pem) {
        Ok(chain) => chain,
        Err(e) => return Err(anyhow!("bad certificate: {e}")),
    };
    let key = match private_key(&tls.key_pem) {
        Ok(key) => key,
        Err(e) => return Err(anyhow!("bad key: {e}")),
    };
    let signer = match any_supported_type(&key) {
        Ok(signer) => signer,
        Err(e) => return Err(anyhow!("key_pem is not a usable private key: {e}")),
    };
    let cert_pem: String = chain
        .iter()
        .map(|cert| armor("CERTIFICATE", cert))
        .collect();
    let key_pem = armor(key_label(&key), key.secret_der());
    match CertifiedKey::new(chain, signer).keys_match() {
        Ok(()) => Ok(Tls { cert_pem, key_pem }),
        Err(e) => Err(anyhow!("key_pem does not match the first certificate: {e}")),
    }
}
