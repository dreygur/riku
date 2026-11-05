use super::*;

fn mock_fetch(url: &str) -> Result<String> {
    if url.ends_with("ips-v4") {
        Ok("173.245.48.0/20\n\n103.21.244.0/22\n".to_string())
    } else {
        Ok("2400:cb00::/32\n".to_string())
    }
}

#[test]
fn config_holds_one_allow_line_per_range() {
    let config = cloudflare_allow_config(mock_fetch).unwrap();

    assert!(
        config.contains("allow 173.245.48.0/20;\nallow 103.21.244.0/22;\nallow 2400:cb00::/32;\n"),
        "{config}"
    );
}

#[test]
fn response_lines_with_nginx_syntax_are_skipped() {
    let fetch = |url: &str| {
        if url.ends_with("ips-v4") {
            Ok("10.0.0.1; deny all\n192.0.2.0/24\nallow all".to_string())
        } else {
            mock_fetch(url)
        }
    };

    let config = cloudflare_allow_config(fetch).unwrap();

    assert!(config.contains("allow 192.0.2.0/24;"), "{config}");
    assert!(!config.contains("deny"), "{config}");
    assert!(!config.contains("allow all"), "{config}");
}

#[test]
fn failed_or_empty_list_is_an_error_naming_the_family() {
    let down = |url: &str| {
        if url.ends_with("ips-v6") {
            Err(anyhow!("connection refused"))
        } else {
            mock_fetch(url)
        }
    };
    let empty = |url: &str| {
        if url.ends_with("ips-v4") {
            Ok("\n".to_string())
        } else {
            mock_fetch(url)
        }
    };

    let down_error = cloudflare_allow_config(down).unwrap_err().to_string();
    let empty_error = cloudflare_allow_config(empty).unwrap_err().to_string();

    assert!(down_error.contains("IPv6"), "{down_error}");
    assert!(empty_error.contains("IPv4"), "{empty_error}");
}
