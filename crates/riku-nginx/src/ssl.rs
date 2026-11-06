//! SSL certificate management for nginx.
//!
//! Obtains certificates via acme.sh (Let's Encrypt) or generates
//! a self-signed fallback certificate using openssl. acme.sh writes http-01
//! challenge files into `RikuPaths::acme_root` (`/var/lib/riku/acme`), which
//! the nginx worker reads through its group, never the deploy home.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::{anyhow, Result};

const KEY_FILE_MODE: u32 = 0o600;
const ACME_UMASK: libc::mode_t = 0o027;

/// Make a private key readable by its owner only. nginx's master process
/// runs as root and reads it before dropping to the worker user, so nothing
/// else needs to.
fn restrict_key(key_path: &std::path::Path) -> Result<()> {
    match fs::set_permissions(key_path, fs::Permissions::from_mode(KEY_FILE_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", key_path.display())),
    }
}

/// Ensure SSL certificates exist for an app.
/// First tries to use acme.sh (Let's Encrypt), falls back to self-signed cert.
/// The key ends up `0600` whichever path made it, an existing one included.
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

/// acme.sh run with umask `027`, so the challenge files it writes in the
/// webroot are readable by the webroot's group (the nginx worker's) and by
/// nobody else, whatever umask riku itself runs under.
fn acme_sh_command(acme_sh: &std::path::Path) -> std::process::Command {
    use std::os::unix::process::CommandExt;

    let mut command = std::process::Command::new(acme_sh);
    // SAFETY: umask is async-signal-safe and touches no memory, so it may
    // run between fork and exec.
    unsafe {
        command.pre_exec(|| {
            libc::umask(ACME_UMASK);
            Ok(())
        });
    }
    command
}

/// The acme.sh argv that issues `domain` for `app`. `--reloadcmd` makes
/// acme.sh run `riku router-renew` after each renewal, which sends the new
/// files to the router helper. `app` reaches a shell there, which is why
/// [`ensure_ssl_certificates`] accepts only plain names.
fn acme_issue_args(app: &str, domain: &str, paths: &crate::config::RikuPaths) -> Vec<String> {
    let nginx_file = |suffix: &str| {
        paths
            .nginx_root
            .join(format!("{app}.{suffix}"))
            .to_string_lossy()
            .into_owned()
    };
    vec![
        "--issue".to_string(),
        "-d".to_string(),
        domain.to_string(),
        "-w".to_string(),
        paths.acme_root.to_string_lossy().into_owned(),
        "--server".to_string(),
        "letsencrypt.org".to_string(),
        "--key-file".to_string(),
        nginx_file("key"),
        "--cert-file".to_string(),
        nginx_file("crt"),
        "--fullchain-file".to_string(),
        nginx_file("fullchain.crt"),
        "--reloadcmd".to_string(),
        format!("{} router-renew {app}", crate::util::ROOT_BINARY),
    ]
}

/// Attempt to issue certificates via acme.sh. Returns `Some(true)` on success,
/// `None` if issuance failed for all domains (caller should fall back).
fn try_acme_certificates(
    app: &str,
    domains: &[String],
    paths: &crate::config::RikuPaths,
    acme_sh: &std::path::Path,
) -> Result<Option<bool>> {
    for domain in domains {
        let result = acme_sh_command(acme_sh)
            .args(acme_issue_args(app, domain, paths))
            .output();

        match result {
            Ok(output) if output.status.success() => {
                crate::util::echo(
                    &format!("-----> Obtained Let's Encrypt certificate for '{}'", domain),
                    "green",
                );
                // Lets destroy find the certificate. Kept out of the webroot,
                // which nginx serves.
                let acme_domain_dir = paths.acme_sh_root.join(domain);
                let link = paths.acme_cert_links.join(app);
                if acme_domain_dir.exists() && !link.exists() {
                    let _ = std::os::unix::fs::symlink(&acme_domain_dir, link);
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_paths(tmp: &TempDir) -> crate::config::RikuPaths {
        let paths = crate::config::RikuPaths::for_tests(tmp.path());
        std::fs::create_dir_all(&paths.nginx_root).unwrap();
        std::fs::create_dir_all(&paths.acme_root).unwrap();
        paths
    }

    #[test]
    fn an_existing_key_is_kept_and_made_owner_only() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let key = paths.nginx_root.join("myapp.key");
        std::fs::write(&key, "key-data").unwrap();
        std::fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::write(paths.nginx_root.join("myapp.crt"), "crt-data").unwrap();

        let domains = vec!["example.com".to_string()];
        assert!(ensure_ssl_certificates("myapp", &domains, &paths).unwrap());

        assert_eq!(std::fs::read_to_string(&key).unwrap(), "key-data");
        let mode = fs::metadata(&key).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "private key is mode {mode:o}");
    }

    #[test]
    fn a_self_signed_pair_retries_acme_with_the_renew_hook_and_survives_a_failure() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        std::fs::create_dir_all(&paths.acme_sh_root).unwrap();
        let argv_log = tmp.path().join("argv.log");
        let acme_sh = paths.acme_sh_root.join("acme.sh");
        std::fs::write(
            &acme_sh,
            format!("#!/bin/sh\necho \"$@\" > {}\nexit 1\n", argv_log.display()),
        )
        .unwrap();
        std::fs::set_permissions(&acme_sh, fs::Permissions::from_mode(0o755)).unwrap();
        let key = paths.nginx_root.join("myapp.key");
        std::fs::write(&key, "self-signed-key").unwrap();
        std::fs::write(paths.nginx_root.join("myapp.crt"), "self-signed-crt").unwrap();

        let domains = vec!["example.com".to_string()];
        assert!(ensure_ssl_certificates("myapp", &domains, &paths).unwrap());

        let argv = std::fs::read_to_string(&argv_log).expect("acme.sh was not run");
        assert!(
            argv.contains("--reloadcmd /usr/local/bin/riku router-renew myapp"),
            "acme.sh argv was: {argv}"
        );
        assert_eq!(std::fs::read_to_string(&key).unwrap(), "self-signed-key");
    }

    #[test]
    fn rejects_an_app_name_that_could_reach_the_reload_shell() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let domains = vec!["example.com".to_string()];

        let err = ensure_ssl_certificates("app; touch /tmp/x", &domains, &paths).unwrap_err();

        assert!(err.to_string().contains("not a valid app name"), "{err}");
    }

    #[test]
    #[ignore = "requires openssl binary on PATH"]
    fn test_generate_self_signed_cert_produces_files() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);

        let domains = vec!["localhost".to_string()];
        let result = ensure_ssl_certificates("testapp", &domains, &paths);
        assert!(result.is_ok());
        assert!(paths.nginx_root.join("testapp.key").exists());
        assert!(paths.nginx_root.join("testapp.crt").exists());
    }
}
