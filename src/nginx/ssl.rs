//! SSL certificate management for nginx.
//!
//! Obtains certificates via acme.sh (Let's Encrypt) or generates
//! a self-signed fallback certificate using openssl.

use anyhow::Result;

/// Ensure SSL certificates exist for an app.
/// First tries to use acme.sh (Let's Encrypt), falls back to self-signed cert.
pub fn ensure_ssl_certificates(
    app: &str,
    domains: &[String],
    paths: &crate::config::RikuPaths,
) -> Result<bool> {
    let key_path = paths.nginx_root.join(format!("{}.key", app));
    let crt_path = paths.nginx_root.join(format!("{}.crt", app));

    // If certs already exist and are valid, don't regenerate
    if key_path.exists() && crt_path.exists() {
        return Ok(true);
    }

    // Try acme.sh first
    let acme_sh = paths.acme_root.join("acme.sh");
    if acme_sh.exists() {
        if let Some(issued) = try_acme_certificates(app, domains, paths, &acme_sh, &key_path, &crt_path)? {
            return Ok(issued);
        }
    }

    // Fall back to self-signed certificate
    generate_self_signed_certificate(app, domains, &key_path, &crt_path)
}

/// Attempt to issue certificates via acme.sh. Returns `Some(true)` on success,
/// `None` if issuance failed for all domains (caller should fall back).
fn try_acme_certificates(
    app: &str,
    domains: &[String],
    paths: &crate::config::RikuPaths,
    acme_sh: &std::path::Path,
    key_path: &std::path::Path,
    crt_path: &std::path::Path,
) -> Result<Option<bool>> {
    for domain in domains {
        let result = std::process::Command::new(acme_sh)
            .args([
                "--issue",
                "-d",
                domain,
                "-w",
                &paths.acme_www.to_string_lossy(),
                "--server",
                "letsencrypt.org",
                "--key-file",
                &key_path.to_string_lossy(),
                "--cert-file",
                &crt_path.to_string_lossy(),
                "--fullchain-file",
                &paths
                    .nginx_root
                    .join(format!("{}.fullchain.crt", app))
                    .to_string_lossy(),
            ])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                crate::util::echo(
                    &format!("-----> Obtained Let's Encrypt certificate for '{}'", domain),
                    "green",
                );
                // Create symlink for ACME_WWW
                let acme_domain_dir = paths.acme_root.join(domain);
                if acme_domain_dir.exists() && !paths.acme_www.join(app).exists() {
                    let _ =
                        std::os::unix::fs::symlink(&acme_domain_dir, paths.acme_www.join(app));
                }
                return Ok(Some(true));
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                crate::util::echo(
                    &format!("-----> Let's Encrypt certificate issue failed: {}", stderr),
                    "yellow",
                );
            }
            Err(e) => {
                crate::util::echo(&format!("-----> acme.sh execution failed: {}", e), "yellow");
            }
        }
    }
    Ok(None)
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
