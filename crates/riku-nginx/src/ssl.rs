//! SSL certificate management for nginx.
//!
//! Obtains a certificate via acme.sh (Let's Encrypt, see [`crate::acme`]) or
//! generates a self-signed fallback certificate using openssl.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::{anyhow, Result};

use crate::acme::{install_renew_hook, try_acme_certificates};

const KEY_FILE_MODE: u32 = 0o600;

/// Make a private key readable by its owner only. nginx's master process
/// runs as root and reads it before dropping to the worker user, so nothing
/// else needs to.
pub(crate) fn restrict_key(key_path: &std::path::Path) -> Result<()> {
    match fs::set_permissions(key_path, fs::Permissions::from_mode(KEY_FILE_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", key_path.display())),
    }
}

/// Generate a self-signed SSL certificate via openssl.
fn generate_self_signed_certificate(
    app: &str,
    domains: &[String],
    key_path: &std::path::Path,
    crt_path: &std::path::Path,
) -> Result<bool> {
    crate::util::echo("-----> Generating self-signed SSL certificate", "yellow");

    let subject = format!(
        "/C=US/ST=NA/L=NA/O=Riku/OU=Self-Signed/CN={}",
        domains.first().unwrap_or(&app.to_string())
    );

    let result = std::process::Command::new("openssl")
        .args([
            "req",
            "-newkey",
            "rsa:4096",
            "-days",
            "365",
            "-nodes",
            "-x509",
            "-subj",
            &subject,
            "-keyout",
            &key_path.to_string_lossy(),
            "-out",
            &crt_path.to_string_lossy(),
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            crate::util::echo(
                &format!("-----> Generated self-signed certificate for '{}'", app),
                "green",
            );
            Ok(true)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!(
                "Failed to generate self-signed certificate: {}",
                stderr
            ))
        }
        Err(e) => Err(anyhow::anyhow!("Failed to run openssl: {}", e)),
    }
}

/// Ensure SSL certificates exist for an app.
/// First tries to use acme.sh (Let's Encrypt), falls back to self-signed cert.
/// The key ends up `0600` whichever path made it, an existing one included.
/// `app` reaches the acme.sh reload shell, which is why only plain names
/// are accepted.
#[allow(clippy::question_mark)]
pub fn ensure_ssl_certificates(
    app: &str,
    domains: &[String],
    paths: &crate::config::RikuPaths,
) -> Result<bool> {
    match crate::util::validate_app_name(app) {
        Ok(checked) if checked == app => {}
        _ => return Err(anyhow!("'{app}' is not a valid app name for a certificate")),
    }
    let key_path = paths.nginx_root.join(format!("{}.key", app));
    let crt_path = paths.nginx_root.join(format!("{}.crt", app));
    let chain_path = paths.nginx_root.join(format!("{}.fullchain.crt", app));
    let have_pair = key_path.exists() && crt_path.exists();
    let acme_sh = paths.acme_sh_root.join("acme.sh");

    if have_pair && (chain_path.exists() || !acme_sh.exists()) {
        if acme_sh.exists() {
            install_renew_hook(app, domains, paths, &acme_sh);
        }
        if let Err(e) = restrict_key(&key_path) {
            return Err(e);
        }
        return Ok(true);
    }

    if acme_sh.exists() {
        let issued = match try_acme_certificates(app, domains, paths, &acme_sh) {
            Ok(issued) => issued,
            Err(e) => return Err(e),
        };
        if let Some(issued) = issued {
            if let Err(e) = restrict_key(&key_path) {
                return Err(e);
            }
            return Ok(issued);
        }
    }

    if have_pair {
        if let Err(e) = restrict_key(&key_path) {
            return Err(e);
        }
        return Ok(true);
    }
    let generated = match generate_self_signed_certificate(app, domains, &key_path, &crt_path) {
        Ok(generated) => generated,
        Err(e) => return Err(e),
    };
    if let Err(e) = restrict_key(&key_path) {
        return Err(e);
    }
    Ok(generated)
}

#[cfg(test)]
#[path = "ssl_tests.rs"]
mod tests;
