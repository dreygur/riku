use anyhow::Result;
use std::fs;

use crate::config::RikuPaths;
use crate::util::{echo, ensure_path_within, exit_if_invalid};

/// Destroy an app — remove directories and config files, preserve data/cache.
pub fn cmd_destroy(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    // Remove app, git, env, log directories
    for dir in [
        &paths.app_root,
        &paths.git_root,
        &paths.env_root,
        &paths.log_root,
    ] {
        let p = dir.join(&app);
        if p.exists() {
            echo(&format!("--> Removing folder '{}'", p.display()), "yellow");
            fs::remove_dir_all(&p)?;
        }
    }

    // Remove worker config files (*.ini and *.toml)
    for dir in [&paths.workers_available, &paths.workers_enabled] {
        for ext in &["ini", "toml"] {
            let pattern = dir.join(format!("{}-*.{}", app, ext));
            if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
                for entry in entries.flatten() {
                    echo(
                        &format!("--> Removing file '{}'", entry.display()),
                        "yellow",
                    );
                    fs::remove_file(&entry)?;
                }
            }
        }
    }

    // Remove nginx configuration and associated files
    echo(
        &format!("--> Removing nginx config for '{}'", app),
        "yellow",
    );
    crate::nginx::remove_nginx_config(&app, paths)?;

    // Remove ACME certs if they exist
    let acme_link = paths.acme_www.join(&app);
    if acme_link.exists() {
        let acme_certs = fs::canonicalize(&acme_link).unwrap_or_else(|_| acme_link.clone());
        if acme_certs.exists() {
            if ensure_path_within(&acme_certs, &paths.riku_root).is_ok()
                || ensure_path_within(&acme_certs, &paths.acme_root).is_ok()
            {
                echo(
                    &format!("--> Removing folder '{}'", acme_certs.display()),
                    "yellow",
                );
                fs::remove_dir_all(&acme_certs)?;
            } else {
                echo(
                    &format!(
                        "WARNING: ACME cert path '{}' points outside expected directories, skipping removal",
                        acme_certs.display()
                    ),
                    "yellow",
                );
            }
        }
        echo(
            &format!("--> Removing file '{}'", acme_link.display()),
            "yellow",
        );
        let _ = fs::remove_file(&acme_link);
    }

    // Preserve data and cache directories
    for dir in [&paths.data_root, &paths.cache_root] {
        let p = dir.join(&app);
        if p.exists() {
            echo(&format!("==> Preserving folder '{}'", p.display()), "red");
        }
    }

    Ok(())
}
