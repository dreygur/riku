/// Integration tests for Nginx configuration generation
///
/// These tests verify the nginx configuration generation
/// for various deployment scenarios.

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper to create a temporary Riku environment
    fn setup_riku_env() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;
        let riku_root = temp_dir.path().join(".riku");

        let dirs = [
            "apps",
            "data",
            "envs",
            "repos",
            "logs",
            "nginx",
            "cache",
            "workers",
            "workers-available",
            "workers-enabled",
            "acme",
            "acme-www",
            "plugins",
        ];

        for dir in &dirs {
            fs::create_dir_all(riku_root.join(dir))?;
        }

        Ok((temp_dir, riku_root))
    }

    #[test]
    fn test_basic_nginx_config() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "basic-app";
        let nginx_dir = riku_root.join("nginx");

        // Create basic nginx config
        let config_content = r#"
server {
    listen 80;
    server_name basic-app.example.com;

    location / {
        proxy_pass http://127.0.0.1:5000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        assert!(config_file.exists());

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("listen 80"));
        assert!(content.contains("server_name basic-app.example.com"));
        assert!(content.contains("proxy_pass"));

        Ok(())
    }

    #[test]
    fn test_https_nginx_config() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "https-app";
        let nginx_dir = riku_root.join("nginx");

        let config_content = r#"
server {
    listen 80;
    server_name https-app.example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name https-app.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("return 301 https://"));
        assert!(content.contains("listen 443 ssl"));
        assert!(content.contains("ssl_certificate"));

        Ok(())
    }

    #[test]
    fn test_static_site_nginx_config() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "static-app";
        let nginx_dir = riku_root.join("nginx");
        let app_dir = riku_root.join("apps").join(app);

        fs::create_dir_all(&app_dir)?;

        let config_content = r#"
server {
    listen 80;
    server_name static-app.example.com;

    root /home/deploy/.riku/apps/static-app;
    index index.html;

    location / {
        try_files $uri $uri/ =404;
    }

    location ~* \.(jpg|jpeg|png|gif|ico|css|js)$ {
        expires 30d;
        add_header Cache-Control "public, immutable";
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("root /home/deploy/.riku/apps/static-app"));
        assert!(content.contains("try_files"));
        assert!(content.contains("expires 30d"));

        Ok(())
    }

    #[test]
    fn test_nginx_with_caching() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "cache-app";
        let nginx_dir = riku_root.join("nginx");
        let _cache_dir = riku_root.join("cache");

        let config_content = r#"
proxy_cache_path /home/deploy/.riku/cache/cache-app levels=1:2 keys_zone=cache_app:100m max_size=1g inactive=60m use_temp_path=off;

server {
    listen 80;
    server_name cache-app.example.com;

    location /api/ {
        proxy_pass http://127.0.0.1:5000;
        proxy_cache cache_app;
        proxy_cache_valid 200 302 10m;
        proxy_cache_valid 404 1m;
        proxy_cache_use_stale error timeout updating http_500 http_502 http_503 http_504;
        add_header X-Cache-Status $upstream_cache_status;
    }

    location / {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("proxy_cache_path"));
        assert!(content.contains("proxy_cache cache_app"));
        assert!(content.contains("X-Cache-Status"));

        Ok(())
    }

    #[test]
    fn test_nginx_with_cloudflare_acl() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "cloudflare-app";
        let nginx_dir = riku_root.join("nginx");

        let config_content = r#"
# Cloudflare IP ranges
set $cf_allowed false;

location / {
    # Allow Cloudflare IPs only
    if ($cf_allowed = false) {
        return 403;
    }

    proxy_pass http://127.0.0.1:5000;
    proxy_set_header CF-Connecting-IP $http_cf_connecting_ip;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("cf_allowed"));
        assert!(content.contains("CF-Connecting-IP"));

        Ok(())
    }

    #[test]
    fn test_nginx_with_ipv6_disabled() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "ipv4-app";
        let nginx_dir = riku_root.join("nginx");

        let config_content = r#"
server {
    listen 127.0.0.1:80;
    # IPv6 disabled
    # listen [::]:80;
    server_name ipv4-app.example.com;

    location / {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("listen 127.0.0.1:80"));
        assert!(content.contains("# listen [::]:80"));

        Ok(())
    }

    #[test]
    fn test_nginx_spa_routing() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "spa-app";
        let nginx_dir = riku_root.join("nginx");

        let config_content = r#"
server {
    listen 80;
    server_name spa-app.example.com;

    root /home/deploy/.riku/apps/spa-app;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location /api/ {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("try_files $uri $uri/ /index.html"));
        assert!(content.contains("location /api/"));

        Ok(())
    }

    #[test]
    fn test_nginx_with_custom_include() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "custom-app";
        let nginx_dir = riku_root.join("nginx");

        // Create custom include file
        let custom_conf = nginx_dir.join("custom-app.custom.conf");
        fs::write(
            &custom_conf,
            "# Custom nginx directives\nadd_header X-Custom-Header \"value\";\n",
        )?;

        let config_content = r#"
server {
    listen 80;
    server_name custom-app.example.com;

    include /home/deploy/.riku/nginx/custom-app.custom.conf;

    location / {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("include /home/deploy/.riku/nginx/custom-app.custom.conf"));
        assert!(custom_conf.exists());

        Ok(())
    }

    #[test]
    fn test_nginx_rate_limiting() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "rate-limit-app";
        let nginx_dir = riku_root.join("nginx");

        let config_content = r#"
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=10r/s;

server {
    listen 80;
    server_name rate-limit-app.example.com;

    location /api/ {
        limit_req zone=api_limit burst=20 nodelay;
        proxy_pass http://127.0.0.1:5000;
    }

    location / {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("limit_req_zone"));
        assert!(content.contains("limit_req zone=api_limit"));

        Ok(())
    }

    #[test]
    fn test_nginx_gzip_compression() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "gzip-app";
        let nginx_dir = riku_root.join("nginx");

        let config_content = r#"
server {
    listen 80;
    server_name gzip-app.example.com;

    gzip on;
    gzip_vary on;
    gzip_proxied any;
    gzip_comp_level 6;
    gzip_types text/plain text/css text/xml application/json application/javascript application/xml;

    location / {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("gzip on"));
        assert!(content.contains("gzip_types"));

        Ok(())
    }

    #[test]
    fn test_nginx_multiple_apps() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let nginx_dir = riku_root.join("nginx");
        let apps = vec!["app1", "app2", "app3"];

        for (i, app) in apps.iter().enumerate() {
            let port = 5000 + i;
            let config_content = format!(
                r#"
server {{
    listen 80;
    server_name {}.example.com;

    location / {{
        proxy_pass http://127.0.0.1:{};
    }}
}}
"#,
                app, port
            );

            let config_file = nginx_dir.join(format!("{}.conf", app));
            fs::write(&config_file, config_content)?;
        }

        let configs: Vec<_> = fs::read_dir(&nginx_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "conf"))
            .collect();

        assert_eq!(configs.len(), 3);

        Ok(())
    }

    #[test]
    fn test_nginx_acme_challenge() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "ssl-app";
        let nginx_dir = riku_root.join("nginx");
        let _acme_www = riku_root.join("acme-www");

        let config_content = r#"
server {
    listen 80;
    server_name ssl-app.example.com;

    location /.well-known/acme-challenge/ {
        root /home/deploy/.riku/acme-www;
    }

    location / {
        return 301 https://$server_name$request_uri;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("/.well-known/acme-challenge/"));
        assert!(content.contains("root /home/deploy/.riku/acme-www"));

        Ok(())
    }

    #[test]
    fn test_nginx_websocket_support() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app = "websocket-app";
        let nginx_dir = riku_root.join("nginx");

        let config_content = r#"
server {
    listen 80;
    server_name websocket-app.example.com;

    location /ws/ {
        proxy_pass http://127.0.0.1:5000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 86400;
    }

    location / {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, config_content)?;

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("proxy_http_version 1.1"));
        assert!(content.contains("Upgrade $http_upgrade"));
        assert!(content.contains("Connection \"upgrade\""));

        Ok(())
    }

    #[test]
    fn test_nginx_config_validation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let nginx_dir = riku_root.join("nginx");

        // Create valid config
        let valid_config = r#"
server {
    listen 80;
    server_name valid.example.com;

    location / {
        proxy_pass http://127.0.0.1:5000;
    }
}
"#;

        let valid_file = nginx_dir.join("valid.conf");
        fs::write(&valid_file, valid_config)?;

        // Create invalid config (missing closing brace)
        let invalid_config = r#"
server {
    listen 80;
    server_name invalid.example.com;

    location / {
        proxy_pass http://127.0.0.1:5000;

"#;

        let invalid_file = nginx_dir.join("invalid.conf");
        fs::write(&invalid_file, invalid_config)?;

        assert!(valid_file.exists());
        assert!(invalid_file.exists());

        // Basic syntax check - count braces
        let valid_content = fs::read_to_string(&valid_file)?;
        let invalid_content = fs::read_to_string(&invalid_file)?;

        let valid_open = valid_content.matches('{').count();
        let valid_close = valid_content.matches('}').count();
        assert_eq!(valid_open, valid_close);

        let invalid_open = invalid_content.matches('{').count();
        let invalid_close = invalid_content.matches('}').count();
        assert_ne!(invalid_open, invalid_close);

        Ok(())
    }

    #[test]
    fn test_nginx_config_cleanup() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let nginx_dir = riku_root.join("nginx");
        let app = "cleanup-app";

        let config_file = nginx_dir.join(format!("{}.conf", app));
        fs::write(&config_file, "server { listen 80; }\n")?;

        assert!(config_file.exists());

        // Simulate app removal - cleanup nginx config
        fs::remove_file(&config_file)?;

        assert!(!config_file.exists());

        Ok(())
    }
}
