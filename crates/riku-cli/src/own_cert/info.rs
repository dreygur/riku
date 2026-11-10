//! Names and expiry of a certificate, read with the `openssl` binary.
//!
//! The dependency tree has no X.509 parser (rustls-webpki checks names but
//! exposes no validity dates), and riku already needs `openssl` for its
//! self-signed fallback, so the leaf is piped to `openssl x509` instead.
//!
//! Security model: only the public leaf certificate goes to the child, on
//! stdin. The program and its arguments are fixed, and the key is never
//! passed.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use chrono::NaiveDateTime;

const DAY_SECS: i64 = 86_400;
/// How many days before the end date `riku doctor` starts to warn.
pub(crate) const WARN_DAYS: i64 = 14;

/// What a certificate says about itself.
#[derive(Debug, PartialEq)]
pub(crate) struct CertInfo {
    /// DNS names of the subject alternative name extension; wildcards kept.
    pub names: Vec<String>,
    /// The end of validity, seconds since the epoch.
    pub expires_at: i64,
}

/// How close a certificate is to its end date.
#[derive(Debug, PartialEq)]
pub(crate) enum Expiry {
    Valid { days: i64 },
    Soon { days: i64 },
    Expired,
}

/// Seconds since the epoch.
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

/// Expired, valid for fewer than [`WARN_DAYS`] days, or valid.
pub(crate) fn classify(expires_at: i64, now: i64) -> Expiry {
    if expires_at < now {
        return Expiry::Expired;
    }
    let days = (expires_at - now) / DAY_SECS;
    if days < WARN_DAYS {
        Expiry::Soon { days }
    } else {
        Expiry::Valid { days }
    }
}

/// Whether one of `names` is `domain`, or a wildcard for exactly one label
/// of it.
pub(crate) fn covers(names: &[String], domain: &str) -> bool {
    let domain = domain.to_ascii_lowercase();
    names.iter().any(|name| {
        let name = name.to_ascii_lowercase();
        match name.strip_prefix("*.") {
            Some(suffix) => domain
                .split_once('.')
                .is_some_and(|(label, rest)| !label.is_empty() && rest == suffix),
            None => name == domain,
        }
    })
}

/// Read the output of `openssl x509 -noout -enddate -ext subjectAltName`.
pub(crate) fn parse_openssl(output: &str) -> Result<CertInfo> {
    let end = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("notAfter="));
    let Some(end) = end else {
        return Err(anyhow!("openssl printed no end date"));
    };
    let expires_at = match NaiveDateTime::parse_from_str(end.trim(), "%b %e %H:%M:%S %Y GMT") {
        Ok(at) => at.and_utc().timestamp(),
        Err(e) => return Err(anyhow!("cannot read the end date '{end}': {e}")),
    };
    let names = output
        .lines()
        .filter(|line| line.contains("DNS:"))
        .flat_map(|line| line.split(','))
        .filter_map(|part| part.trim().strip_prefix("DNS:"))
        .map(str::to_string)
        .collect();
    Ok(CertInfo { names, expires_at })
}

/// Names and end date of the first certificate in `cert_pem`.
pub(crate) fn read(cert_pem: &str) -> Result<CertInfo> {
    let mut child = match Command::new("openssl")
        .args(["x509", "-noout", "-enddate", "-ext", "subjectAltName"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return Err(anyhow!("cannot run openssl: {e}")),
    };
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(cert_pem.as_bytes()) {
            return Err(anyhow!("cannot send the certificate to openssl: {e}"));
        }
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("openssl failed: {e}")),
    };
    if !output.status.success() {
        return Err(anyhow!(
            "openssl cannot read the certificate: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_openssl(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
#[path = "info_tests.rs"]
mod tests;
