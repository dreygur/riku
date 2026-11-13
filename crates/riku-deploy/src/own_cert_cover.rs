//! Which of an app's domains the certificate its owner supplied covers.
//!
//! The router serves that certificate for every name of the app, so a name
//! outside it gets a browser warning on HTTPS. The push output lists HTTPS
//! only for the covered names. rustls-webpki checks each name the way a
//! client would, from the subject alternative names, wildcards included.
//!
//! Security model: the chain is the owner's own file under `nginx_root`,
//! already validated when `riku certs set` stored it. Only the first
//! certificate is parsed, and a file that does not parse covers nothing.

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName};
use webpki::EndEntityCert;

use crate::config::RikuPaths;

fn leaf(paths: &RikuPaths, app: &str) -> Option<CertificateDer<'static>> {
    let chain = paths.nginx_root.join(format!("{app}.fullchain.crt"));
    let mut certs = match CertificateDer::pem_file_iter(chain) {
        Ok(certs) => certs,
        Err(_) => return None,
    };
    match certs.next() {
        Some(Ok(cert)) => Some(cert),
        _ => None,
    }
}

fn names_domain(cert: &EndEntityCert<'_>, domain: &str) -> bool {
    match ServerName::try_from(domain) {
        Ok(name @ ServerName::DnsName(_)) => cert.verify_is_valid_for_subject_name(&name).is_ok(),
        _ => false,
    }
}

/// The entries of `domains` that `app`'s own certificate is valid for, in
/// order. Empty when the chain is missing or does not parse.
pub(crate) fn covered_domains(paths: &RikuPaths, app: &str, domains: &[String]) -> Vec<String> {
    let Some(der) = leaf(paths, app) else {
        return Vec::new();
    };
    let cert = match EndEntityCert::try_from(&der) {
        Ok(cert) => cert,
        Err(_) => return Vec::new(),
    };
    domains
        .iter()
        .filter(|domain| names_domain(&cert, domain))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "own_cert_cover_tests.rs"]
mod tests;
