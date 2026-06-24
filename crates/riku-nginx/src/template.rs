//! Nginx config template rendering and config file installation.
//!
//! Registers Tera templates, selects the right template for an app's config,
//! renders the file, and manages the /etc/nginx/sites-enabled/ symlink.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::context::build_context;
use super::sanitize::sanitize_env_for_nginx;
use super::validate::validate_nginx_config;

/// Use a custom nginx configuration file from the app directory.
pub(super) fn use_custom_nginx_config(
    custom_path: &Path,
    app: &str,
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    let config_content = fs::read_to_string(custom_path)?;
    let config_file = paths.nginx_root.join(format!("{}.conf", app));
    crate::util::write_atomic(&config_file, config_content.as_bytes())?;
    validate_nginx_config(&config_file)?;
    crate::util::echo(
        &format!("-----> Custom nginx config installed for '{}'", app),
        "green",
    );
    Ok(())
}

/// Generate nginx configuration from template.
pub(super) fn generate_nginx_config_from_template(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    let mut tera = tera::Tera::default();
    register_templates(&mut tera)?;

    let env = &sanitize_env_for_nginx(env);
    let context = build_context(app, app_path, env, paths)?;
    let template_name = select_template(env);

    let config_content = tera.render(template_name, &context)?;
    let config_file = paths.nginx_root.join(format!("{}.conf", app));
    crate::util::write_atomic(&config_file, config_content.as_bytes())?;
    validate_nginx_config(&config_file)?;

    install_nginx_symlink(&config_file, app);

    Ok(())
}

/// Register all nginx Tera templates.
fn register_templates(tera: &mut tera::Tera) -> Result<()> {
    let templates = [
        (
            "nginx.conf.tera",
            include_str!("../../../templates/nginx.conf.tera"),
        ),
        (
            "nginx_https_only.conf.tera",
            include_str!("../../../templates/nginx_https_only.conf.tera"),
        ),
        (
            "nginx_common.conf.tera",
            include_str!("../../../templates/nginx_common.conf.tera"),
        ),
        (
            "nginx_portmap.conf.tera",
            include_str!("../../../templates/nginx_portmap.conf.tera"),
        ),
        (
            "nginx_acme_firstrun.conf.tera",
            include_str!("../../../templates/nginx_acme_firstrun.conf.tera"),
        ),
        (
            "nginx_static.conf.tera",
            include_str!("../../../templates/nginx_static.conf.tera"),
        ),
        (
            "nginx_cache.conf.tera",
            include_str!("../../../templates/nginx_cache.conf.tera"),
        ),
        (
            "nginx_proxy.conf.tera",
            include_str!("../../../templates/nginx_proxy.conf.tera"),
        ),
        (
            "nginx_wsgi.conf.tera",
            include_str!("../../../templates/nginx_wsgi.conf.tera"),
        ),
    ];

    for (name, content) in &templates {
        tera.add_raw_template(name, content)?;
    }
    Ok(())
}

/// Choose the appropriate template based on environment flags.
fn select_template(env: &HashMap<String, String>) -> &'static str {
    if env.contains_key("NGINX_HTTPS_ONLY") {
        "nginx_https_only.conf.tera"
    } else if env.contains_key("NGINX_WSGI") {
        "nginx_wsgi.conf.tera"
    } else if env.contains_key("NGINX_PORTMAP") {
        "nginx_portmap.conf.tera"
    } else if env.contains_key("NGINX_STATIC") {
        "nginx_static.conf.tera"
    } else {
        "nginx.conf.tera"
    }
}

/// Create or update the /etc/nginx/sites-enabled/ symlink and reload nginx.
fn install_nginx_symlink(config_file: &Path, app: &str) {
    let nginx_sites_enabled = Path::new("/etc/nginx/sites-enabled");
    if !nginx_sites_enabled.exists() {
        return;
    }

    let symlink_path = nginx_sites_enabled.join(format!("{}.conf", app));

    if symlink_path.symlink_metadata().is_ok() {
        if let Err(e) = fs::remove_file(&symlink_path) {
            tracing::warn!(
                "could not remove old nginx symlink {:?}: {}",
                symlink_path,
                e
            );
        }
    }

    if let Err(e) = std::os::unix::fs::symlink(config_file, &symlink_path) {
        tracing::warn!("could not create nginx symlink {:?}: {}", symlink_path, e);
    } else {
        super::reload_nginx();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── select_template ────────────────────────────────────────────────────

    #[test]
    fn test_select_template_default_when_no_flags() {
        let env = HashMap::new();
        assert_eq!(select_template(&env), "nginx.conf.tera");
    }

    #[test]
    fn test_select_template_https_only() {
        let mut env = HashMap::new();
        env.insert("NGINX_HTTPS_ONLY".to_string(), "true".to_string());
        assert_eq!(select_template(&env), "nginx_https_only.conf.tera");
    }

    #[test]
    fn test_select_template_wsgi() {
        let mut env = HashMap::new();
        env.insert("NGINX_WSGI".to_string(), "true".to_string());
        assert_eq!(select_template(&env), "nginx_wsgi.conf.tera");
    }

    #[test]
    fn test_select_template_portmap() {
        let mut env = HashMap::new();
        env.insert("NGINX_PORTMAP".to_string(), "true".to_string());
        assert_eq!(select_template(&env), "nginx_portmap.conf.tera");
    }

    #[test]
    fn test_select_template_static() {
        let mut env = HashMap::new();
        env.insert("NGINX_STATIC".to_string(), "true".to_string());
        assert_eq!(select_template(&env), "nginx_static.conf.tera");
    }

    #[test]
    fn test_select_template_https_only_takes_precedence_over_wsgi() {
        let mut env = HashMap::new();
        env.insert("NGINX_HTTPS_ONLY".to_string(), "true".to_string());
        env.insert("NGINX_WSGI".to_string(), "true".to_string());
        // HTTPS_ONLY is checked first
        assert_eq!(select_template(&env), "nginx_https_only.conf.tera");
    }

    // ── register_templates ────────────────────────────────────────────────

    #[test]
    fn test_register_templates_succeeds() {
        let mut tera = tera::Tera::default();
        let result = register_templates(&mut tera);
        assert!(result.is_ok());
    }

    #[test]
    fn test_all_expected_templates_registered() {
        let mut tera = tera::Tera::default();
        register_templates(&mut tera).unwrap();
        let names: Vec<String> = tera.get_template_names().map(str::to_string).collect();
        assert!(names.contains(&"nginx.conf.tera".to_string()));
        assert!(names.contains(&"nginx_https_only.conf.tera".to_string()));
        assert!(names.contains(&"nginx_portmap.conf.tera".to_string()));
        assert!(names.contains(&"nginx_static.conf.tera".to_string()));
        assert!(names.contains(&"nginx_wsgi.conf.tera".to_string()));
    }
}
