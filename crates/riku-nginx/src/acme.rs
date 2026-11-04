//! The first-run ACME challenge config.
//!
//! It is written as [`ACME_CONFIG_FILE`], a name that does not end in
//! `.conf`, so the host include never loads it and no app's `<app>.conf`
//! can collide with it. It stays off until its template gets the
//! `RIKU_DOMAINS` it needs; today rendering always fails on that.

use anyhow::{anyhow, Result};

use super::install::install_checked;
use crate::config::RikuPaths;

/// File name of the ACME config inside `nginx_root`.
pub const ACME_CONFIG_FILE: &str = "acme-challenge.conf.disabled";

/// Render the ACME challenge config and, once `nginx -t` accepts it, write
/// it to [`ACME_CONFIG_FILE`].
pub fn generate_acme_nginx_config(paths: &RikuPaths) -> Result<()> {
    let mut tera = tera::Tera::default();
    if let Err(e) = tera.add_raw_template(
        "acme.conf.tera",
        include_str!("../../../templates/nginx_acme_firstrun.conf.tera"),
    ) {
        return Err(anyhow!("cannot load the ACME nginx template: {e}"));
    }

    let mut context = tera::Context::new();
    context.insert("ACME_WWW", &paths.acme_www.to_string_lossy());
    context.insert("RIKU_NGINX_IPV4_ADDRESS", "0.0.0.0");
    context.insert("RIKU_NGINX_IPV6_ADDRESS", "[::]");

    let config_content = match tera.render("acme.conf.tera", &context) {
        Ok(content) => content,
        Err(e) => return Err(anyhow!("cannot render the ACME nginx config: {e}")),
    };

    install_checked(
        &paths.nginx_root.join(ACME_CONFIG_FILE),
        &config_content,
        paths,
    )
}
