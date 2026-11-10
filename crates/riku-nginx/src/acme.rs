//! Let's Encrypt certificates through acme.sh: issuing one certificate for
//! all of an app's domains, and the renew hook that sends renewed files to
//! the router helper.
//!
//! Security: acme.sh writes http-01 challenge files into
//! `RikuPaths::acme_root` (`/var/lib/riku/acme`), which the nginx worker
//! reads through its group, never the deploy home. The app name reaches the
//! `--reloadcmd` shell, so callers pass only validated plain names.

use std::fs;

use anyhow::Result;

const ACME_UMASK: libc::mode_t = 0o027;

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

/// The acme.sh arguments that say where `app`'s files go and what to run
/// after each renewal: `riku router-renew`, which sends the new files to the
/// router helper.
fn acme_output_args(app: &str, paths: &crate::config::RikuPaths) -> Vec<String> {
    let nginx_file = |suffix: &str| {
        paths
            .nginx_root
            .join(format!("{app}.{suffix}"))
            .to_string_lossy()
            .into_owned()
    };
    vec![
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

/// One `--issue` for all of `domains`: acme.sh makes a single certificate
/// with each as a name, filed under the first.
fn acme_issue_args(app: &str, domains: &[String], paths: &crate::config::RikuPaths) -> Vec<String> {
    let mut args = vec!["--issue".to_string()];
    for domain in domains {
        args.push("-d".to_string());
        args.push(domain.clone());
    }
    args.extend([
        "-w".to_string(),
        paths.acme_root.to_string_lossy().into_owned(),
        "--server".to_string(),
        "letsencrypt.org".to_string(),
    ]);
    args.extend(acme_output_args(app, paths));
    args
}

fn acme_install_args(app: &str, domain: &str, paths: &crate::config::RikuPaths) -> Vec<String> {
    let mut args = vec![
        "--install-cert".to_string(),
        "-d".to_string(),
        domain.to_string(),
    ];
    args.extend(acme_output_args(app, paths));
    args
}

/// Store the renew hook for a certificate acme.sh issued before the hook
/// existed, so its renewals reach the router helper. acme.sh files a
/// certificate under one of the app's domains, so the first domain it
/// accepts is the one; a marker file next to the certificate then keeps
/// deploys from repeating it. A failure is reported and retried on the next
/// deploy.
pub(crate) fn install_renew_hook(
    app: &str,
    domains: &[String],
    paths: &crate::config::RikuPaths,
    acme_sh: &std::path::Path,
) {
    let marker = paths.nginx_root.join(format!("{app}.renew-hook"));
    if marker.exists() {
        return;
    }
    let installed = domains.iter().any(|domain| {
        match acme_sh_command(acme_sh)
            .args(acme_install_args(app, domain, paths))
            .output()
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    });
    if !installed {
        crate::util::echo(
            &format!(
                "-----> Could not set the renew hook: acme.sh has no certificate for {}",
                domains.join(", ")
            ),
            "yellow",
        );
        return;
    }
    if let Err(e) = fs::write(&marker, "") {
        crate::util::echo(
            &format!("-----> Could not record the renew hook: {e}"),
            "yellow",
        );
    }
}

/// One acme.sh issue for `domains`; reports a failure and returns false.
fn issue(
    app: &str,
    domains: &[String],
    paths: &crate::config::RikuPaths,
    acme_sh: &std::path::Path,
) -> bool {
    let output = match acme_sh_command(acme_sh)
        .args(acme_issue_args(app, domains, paths))
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            crate::util::echo(&format!("-----> acme.sh execution failed: {e}"), "yellow");
            return false;
        }
    };
    if output.status.success() {
        crate::util::echo(
            &format!(
                "-----> Obtained Let's Encrypt certificate for {}",
                domains.join(", ")
            ),
            "green",
        );
        return true;
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    crate::util::echo(
        &format!("-----> Let's Encrypt certificate issue failed: {stderr}"),
        "yellow",
    );
    false
}

/// Lets destroy find the certificate acme.sh filed under `domain`. Kept out
/// of the webroot, which nginx serves.
fn link_certificate(app: &str, domain: &str, paths: &crate::config::RikuPaths) {
    let acme_domain_dir = paths.acme_sh_root.join(domain);
    let link = paths.acme_cert_links.join(app);
    if acme_domain_dir.exists() && !link.exists() {
        let _ = std::os::unix::fs::symlink(&acme_domain_dir, link);
    }
}

/// Attempt to issue a certificate via acme.sh. Returns `Some(true)` on
/// success, `None` if issuance failed (caller should fall back).
///
/// One certificate names every domain. When that fails, usually because one
/// domain does not point at this server yet, the first domain that passes
/// on its own gets the certificate and the push says which domains are left
/// without one.
pub(crate) fn try_acme_certificates(
    app: &str,
    domains: &[String],
    paths: &crate::config::RikuPaths,
    acme_sh: &std::path::Path,
) -> Result<Option<bool>> {
    let Some(first) = domains.first() else {
        return Ok(None);
    };
    if issue(app, domains, paths, acme_sh) {
        link_certificate(app, first, paths);
        return Ok(Some(true));
    }
    if domains.len() == 1 {
        return Ok(None);
    }
    for domain in domains {
        if !issue(app, std::slice::from_ref(domain), paths, acme_sh) {
            continue;
        }
        link_certificate(app, domain, paths);
        let uncovered: Vec<&str> = domains
            .iter()
            .filter(|other| *other != domain)
            .map(String::as_str)
            .collect();
        crate::util::echo(
            &format!(
                "-----> The certificate covers only '{domain}'; {} failed the Let's Encrypt check and get a certificate name error",
                uncovered.join(", ")
            ),
            "yellow",
        );
        return Ok(Some(true));
    }
    Ok(None)
}
