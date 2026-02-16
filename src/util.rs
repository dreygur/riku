use anyhow::Result;
use colored::Colorize;
use regex::Regex;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::process::{self, Command};
use which::which;

/// Cron regexp matching five time fields followed by a command.
const CRON_REGEXP: &str = r"^((?:(?:\*/)?\d+)|\*) ((?:(?:\*/)?\d+)|\*) ((?:(?:\*/)?\d+)|\*) ((?:(?:\*/)?\d+)|\*) ((?:(?:\*/)?\d+)|\*) (.*)$";

/// Sanitize the app name: only allow alphanumeric, dots, underscores, hyphens.
/// Strip leading slashes, trim trailing whitespace.
pub fn sanitize_app_name(app: &str) -> String {
    let stripped = app.trim_start_matches('/');
    stripped
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Sanitize name, check app dir exists, exit(1) if not.
pub fn exit_if_invalid(app: &str, app_root: &Path) -> Result<String> {
    let app = sanitize_app_name(app);
    if !app_root.join(&app).exists() {
        echo(&format!("Error: app '{}' not found.", app), "red");
        process::exit(1);
    }
    Ok(app)
}

/// Find a free TCP port (entirely at random) by binding to port 0.
pub fn get_free_port(address: &str) -> u16 {
    let bind_addr = format!("{}:0", address);
    let listener = TcpListener::bind(&bind_addr).expect("Failed to bind to address");
    listener.local_addr().unwrap().port()
}

/// Convert a boolean-ish string to a boolean.
#[allow(dead_code)]
pub fn get_boolean(value: &str) -> bool {
    matches!(
        value.to_lowercase().as_str(),
        "1" | "on" | "true" | "enabled" | "yes" | "y"
    )
}

/// Validate and parse a positive integer environment variable.
/// Returns Ok(value) if valid, or Err with a helpful error message.
#[allow(dead_code)]
pub fn parse_positive_int(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| {
            format!(
                "Invalid value for {}: '{}' is not a valid positive integer",
                name, value
            )
        })
        .and_then(|v| {
            if v == 0 {
                Err(format!(
                    "Invalid value for {}: must be greater than 0",
                    name
                ))
            } else {
                Ok(v)
            }
        })
}

/// Validate a Node.js version string.
#[allow(dead_code)]
pub fn validate_node_version(version: &str) -> Result<(), String> {
    let version = version.trim();

    if version.is_empty() {
        return Err("NODE_VERSION cannot be empty".to_string());
    }

    // Basic version format check (e.g., "18.17.0", "18", "18.x")
    let version_regex = Regex::new(r"^\d+(\.\d+)*(-[\w.]+)?$").unwrap();
    if !version_regex.is_match(version) {
        return Err(format!(
            "Invalid NODE_VERSION: '{}' - expected format like '18.17.0' or '18'",
            version
        ));
    }

    Ok(())
}

/// Validate nginx cache configuration.
#[allow(dead_code)]
pub fn validate_nginx_cache_config(
    cache_size: &str,
    cache_time: &str,
    cache_expiry: &str,
) -> Result<(), String> {
    // Validate cache size (1-100 GB)
    let size = cache_size.parse::<u32>().map_err(|_| {
        format!(
            "Invalid NGINX_CACHE_SIZE: '{}' - must be a number between 1 and 100",
            cache_size
        )
    })?;

    if !(1..=100).contains(&size) {
        return Err(format!(
            "Invalid NGINX_CACHE_SIZE: {} - must be between 1 and 100 GB",
            size
        ));
    }

    // Validate cache time (positive integer)
    cache_time.parse::<u32>().map_err(|_| {
        format!(
            "Invalid NGINX_CACHE_TIME: '{}' - must be a positive integer (seconds)",
            cache_time
        )
    })?;

    // Validate cache expiry (positive integer)
    cache_expiry.parse::<u32>().map_err(|_| {
        format!(
            "Invalid NGINX_CACHE_EXPIRY: '{}' - must be a positive integer (seconds)",
            cache_expiry
        )
    })?;

    Ok(())
}

/// Validate environment variables and return warnings/errors.
#[allow(dead_code)]
pub fn validate_env_vars(env: &HashMap<String, String>) -> Vec<String> {
    let mut warnings = Vec::new();

    // Check for deprecated uWSGI variables
    let deprecated_vars = [
        "UWSGI_MAX_REQUESTS",
        "UWSGI_PROCESSES",
        "UWSGI_LISTEN",
        "UWSGI_ENABLE_THREADS",
        "UWSGI_LOG_MAXSIZE",
        "UWSGI_IDLE",
        "UWSGI_GEVENT",
        "UWSGI_ASYNCIO",
        "UWSGI_INCLUDE_FILE",
    ];

    for var in &deprecated_vars {
        if env.contains_key(*var) {
            warnings.push(format!(
                "Warning: {} is deprecated - Riku uses a custom supervisor instead of uWSGI. \
                 Use RIKU_* variables or the SCALING file instead.",
                var
            ));
        }
    }

    // Validate NODE_VERSION if present
    if let Some(version) = env.get("NODE_VERSION") {
        if let Err(e) = validate_node_version(version) {
            warnings.push(e);
        }
    }

    // Validate nginx cache config if present
    if env.contains_key("NGINX_CACHE_PREFIXES") {
        let cache_size = env
            .get("NGINX_CACHE_SIZE")
            .map(|s| s.as_str())
            .unwrap_or("1");
        let cache_time = env
            .get("NGINX_CACHE_TIME")
            .map(|s| s.as_str())
            .unwrap_or("3600");
        let cache_expiry = env
            .get("NGINX_CACHE_EXPIRY")
            .map(|s| s.as_str())
            .unwrap_or("86400");

        if let Err(e) = validate_nginx_cache_config(cache_size, cache_time, cache_expiry) {
            warnings.push(e);
        }
    }

    warnings
}

/// Print environment variable validation warnings.
#[allow(dead_code)]
pub fn print_env_warnings(warnings: &[String]) {
    for warning in warnings {
        echo(warning, "yellow");
    }
}

/// Write key=value config file.
pub fn write_config(filename: &Path, bag: &HashMap<String, String>, separator: &str) -> Result<()> {
    let mut file = fs::File::create(filename)?;
    for (k, v) in bag.iter() {
        writeln!(file, "{}{}{}", k, separator, v)?;
    }
    Ok(())
}

/// Append to ~/.ssh/authorized_keys with SSH restrictions.
/// Set directory permissions to 700, file permissions to 600.
pub fn setup_authorized_keys(ssh_fingerprint: &str, script_path: &str, pubkey: &str) -> Result<()> {
    let home = std::env::var("HOME")?;
    let ssh_dir = Path::new(&home).join(".ssh");
    let authorized_keys = ssh_dir.join("authorized_keys");

    if !ssh_dir.exists() {
        fs::create_dir_all(&ssh_dir)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&authorized_keys)?;

    writeln!(
        file,
        "command=\"FINGERPRINT={} NAME=default {} $SSH_ORIGINAL_COMMAND\",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding {}",
        ssh_fingerprint, script_path, pubkey
    )?;

    // Set permissions: dir 700, file 600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o700))?;
        fs::set_permissions(&authorized_keys, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Parse a Heroku-style Procfile. Skip comments/blanks. Validate cron entries.
/// WSGI trumps web workers. Returns None if file missing.
pub fn parse_procfile(filename: &Path) -> Option<HashMap<String, String>> {
    if !filename.exists() {
        return None;
    }

    let content = fs::read_to_string(filename).ok()?;
    let cron_re = Regex::new(CRON_REGEXP).unwrap();
    let mut workers: HashMap<String, String> = HashMap::new();

    for (line_number, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(colon_pos) = line.find(':') {
            let kind = line[..colon_pos].trim().to_string();
            let command = line[colon_pos + 1..].trim().to_string();

            // Check for cron patterns
            if kind.starts_with("cron") {
                let limits = [59, 24, 31, 12, 7];
                if let Some(caps) = cron_re.captures(&command) {
                    let mut valid = true;
                    for i in 0..limits.len() {
                        let field = &caps[i + 1];
                        let num_str = field.replace("*/", "").replace('*', "1");
                        match num_str.parse::<u32>() {
                            Ok(n) if n > limits[i] => {
                                valid = false;
                                break;
                            }
                            Err(_) => {
                                valid = false;
                                break;
                            }
                            _ => {}
                        }
                    }
                    if !valid {
                        echo(
                            &format!(
                                "Warning: misformatted Procfile entry '{}' at line {}",
                                line, line_number
                            ),
                            "yellow",
                        );
                        continue;
                    }
                } else {
                    echo(
                        &format!(
                            "Warning: misformatted Procfile entry '{}' at line {}",
                            line, line_number
                        ),
                        "yellow",
                    );
                    continue;
                }
            }

            if workers.contains_key(&kind) {
                echo(
                    &format!(
                        "Warning: found multiple {} workers, only the last one will be used.",
                        kind
                    ),
                    "yellow",
                );
            }
            workers.insert(kind, command);
        } else {
            echo(
                &format!(
                    "Warning: misformatted Procfile entry '{}' at line {}",
                    line, line_number
                ),
                "yellow",
            );
        }
    }

    // WSGI trumps regular web workers
    if (workers.contains_key("wsgi")
        || workers.contains_key("jwsgi")
        || workers.contains_key("rwsgi"))
        && workers.contains_key("web") {
            echo(
                "Warning: found both 'wsgi' and 'web' workers, disabling 'web'",
                "yellow",
            );
            workers.remove("web");
        }

    Some(workers)
}

/// Expand shell-style environment variables ($VAR and ${VAR}) in a buffer.
/// If var not found and no default, keep original text. If default given, use it.
pub fn expandvars(buffer: &str, env: &HashMap<String, String>, default: Option<&str>) -> String {
    let re = Regex::new(r"\$(\w+|\{([^}]*)\})").unwrap();
    re.replace_all(buffer, |caps: &regex::Captures| {
        // Group 2 is the braced name (${VAR}), group 1 is $VAR (word chars)
        let var_name = caps
            .get(2)
            .map(|m| m.as_str())
            .unwrap_or_else(|| caps.get(1).unwrap().as_str());

        if let Some(val) = env.get(var_name) {
            val.clone()
        } else {
            match default {
                Some(d) => d.to_string(),
                None => caps.get(0).unwrap().as_str().to_string(),
            }
        }
    })
    .to_string()
}

/// Run shell command, return stdout. Return empty string on failure.
#[allow(dead_code)]
pub fn command_output(cmd: &str) -> String {
    match Command::new("sh").arg("-c").arg(cmd).output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => String::new(),
    }
}

/// Parse KEY=VALUE file with variable interpolation. Skip comments/blanks.
/// On malformat, print error and return empty map.
pub fn parse_settings(
    filename: &Path,
    env: &mut HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    if !filename.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(filename)?;
    for line in content.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Also skip lines that are only whitespace
        if line.trim().is_empty() {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let k = line[..eq_pos].trim().to_string();
            let v = line[eq_pos + 1..].trim().to_string();
            let expanded = expandvars(&v, env, None);
            env.insert(k, expanded);
        } else {
            echo(
                &format!("Error: malformed setting '{}', ignoring file.", line),
                "red",
            );
            return Ok(HashMap::new());
        }
    }

    Ok(env.clone())
}

/// Check all binaries exist via `which`. Print results.
#[allow(dead_code)]
pub fn check_requirements(binaries: &[&str]) -> bool {
    echo(
        &format!("-----> Checking requirements: {:?}", binaries),
        "green",
    );
    let results: Vec<Option<std::path::PathBuf>> = binaries.iter().map(|b| which(b).ok()).collect();
    echo(&format!("{:?}", results), "");

    results.iter().all(|r| r.is_some())
}

/// Print "-----> {kind} app detected." in green, return true.
pub fn found_app(kind: &str) -> bool {
    echo(&format!("-----> {} app detected.", kind), "green");
    true
}

/// Print colored output with different log levels.
/// "green" -> green (info), "yellow" -> yellow (warning), "red" -> stderr red (error), other -> plain.
pub fn echo(msg: &str, color: &str) {
    match color {
        "green" => println!("{}", format!("-----> {}", msg).green()),
        "yellow" => eprintln!("{}", format!(" !     {}", msg).yellow()),
        "red" => eprintln!("{}", format!(" !     {}", msg).red()),
        _ => println!("{}", msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    // --- sanitize_app_name ---

    #[test]
    fn test_sanitize_valid_name() {
        assert_eq!(sanitize_app_name("my-app"), "my-app");
        assert_eq!(sanitize_app_name("my_app.v2"), "my_app.v2");
        assert_eq!(sanitize_app_name("app123"), "app123");
    }

    #[test]
    fn test_sanitize_invalid_chars() {
        assert_eq!(sanitize_app_name("my app!@#"), "myapp");
        assert_eq!(sanitize_app_name("app/name"), "appname");
        assert_eq!(sanitize_app_name("a b c"), "abc");
    }

    #[test]
    fn test_sanitize_leading_slashes() {
        assert_eq!(sanitize_app_name("/my-app"), "my-app");
        assert_eq!(sanitize_app_name("///app"), "app");
        assert_eq!(sanitize_app_name("/"), "");
    }

    #[test]
    fn test_sanitize_trailing_whitespace() {
        assert_eq!(sanitize_app_name("my-app  "), "my-app");
        assert_eq!(sanitize_app_name("app\t"), "app");
    }

    // --- get_boolean ---

    #[test]
    fn test_get_boolean_truthy() {
        for val in &["1", "on", "true", "enabled", "yes", "y"] {
            assert!(get_boolean(val), "expected true for '{}'", val);
        }
    }

    #[test]
    fn test_get_boolean_case_insensitive() {
        assert!(get_boolean("True"));
        assert!(get_boolean("TRUE"));
        assert!(get_boolean("ON"));
        assert!(get_boolean("Yes"));
        assert!(get_boolean("Y"));
        assert!(get_boolean("Enabled"));
    }

    #[test]
    fn test_get_boolean_falsy() {
        assert!(!get_boolean("0"));
        assert!(!get_boolean("off"));
        assert!(!get_boolean("false"));
        assert!(!get_boolean("no"));
        assert!(!get_boolean("n"));
        assert!(!get_boolean(""));
        assert!(!get_boolean("random"));
    }

    // --- expandvars ---

    #[test]
    fn test_expandvars_simple() {
        let mut env = HashMap::new();
        env.insert("HOME".into(), "/home/user".into());
        assert_eq!(expandvars("$HOME/bin", &env, None), "/home/user/bin");
    }

    #[test]
    fn test_expandvars_braced() {
        let mut env = HashMap::new();
        env.insert("APP".into(), "myapp".into());
        assert_eq!(expandvars("${APP}/data", &env, None), "myapp/data");
    }

    #[test]
    fn test_expandvars_missing_no_default() {
        let env = HashMap::new();
        assert_eq!(expandvars("$MISSING", &env, None), "$MISSING");
        assert_eq!(expandvars("${MISSING}", &env, None), "${MISSING}");
    }

    #[test]
    fn test_expandvars_missing_with_default() {
        let env = HashMap::new();
        assert_eq!(expandvars("$MISSING", &env, Some("")), "");
        assert_eq!(expandvars("$MISSING", &env, Some("fallback")), "fallback");
    }

    #[test]
    fn test_expandvars_multiple() {
        let mut env = HashMap::new();
        env.insert("A".into(), "hello".into());
        env.insert("B".into(), "world".into());
        assert_eq!(expandvars("$A $B", &env, None), "hello world");
    }

    // --- parse_procfile ---

    #[test]
    fn test_parse_procfile_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "worker: celery -A tasks").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert_eq!(workers.get("web").unwrap(), "python app.py");
        assert_eq!(workers.get("worker").unwrap(), "celery -A tasks");
    }

    #[test]
    fn test_parse_procfile_comments_and_blanks() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "# This is a comment").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "# Another comment").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers.get("web").unwrap(), "python app.py");
    }

    #[test]
    fn test_parse_procfile_wsgi_trumps_web() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "wsgi: app:application").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(!workers.contains_key("web"));
        assert!(workers.contains_key("wsgi"));
    }

    #[test]
    fn test_parse_procfile_missing_file() {
        let result = parse_procfile(Path::new("/nonexistent/Procfile"));
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_procfile_cron_valid() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: */5 * * * * /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_invalid_value() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 60 * * * * /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(!workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_empty_file() {
        let f = NamedTempFile::new().unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.is_empty());
    }

    // --- parse_settings ---

    #[test]
    fn test_parse_settings_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "PORT=8080").unwrap();
        writeln!(f, "HOST=localhost").unwrap();
        let mut env = HashMap::new();
        let result = parse_settings(f.path(), &mut env).unwrap();
        assert_eq!(result.get("PORT").unwrap(), "8080");
        assert_eq!(result.get("HOST").unwrap(), "localhost");
    }

    #[test]
    fn test_parse_settings_variable_interpolation() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "BASE=/opt/app").unwrap();
        writeln!(f, "DATA=$BASE/data").unwrap();
        let mut env = HashMap::new();
        let result = parse_settings(f.path(), &mut env).unwrap();
        assert_eq!(result.get("DATA").unwrap(), "/opt/app/data");
    }

    #[test]
    fn test_parse_settings_comments() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "# comment").unwrap();
        writeln!(f, "KEY=value").unwrap();
        writeln!(f, "").unwrap();
        let mut env = HashMap::new();
        let result = parse_settings(f.path(), &mut env).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("KEY").unwrap(), "value");
    }

    #[test]
    fn test_parse_settings_malformed() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "KEY=value").unwrap();
        writeln!(f, "BADLINE").unwrap();
        let mut env = HashMap::new();
        let result = parse_settings(f.path(), &mut env).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_settings_missing_file() {
        let mut env = HashMap::new();
        let result = parse_settings(Path::new("/nonexistent/settings"), &mut env).unwrap();
        assert!(result.is_empty());
    }

    // --- get_free_port ---

    #[test]
    fn test_get_free_port_returns_valid_port() {
        let port = get_free_port("127.0.0.1");
        assert!(port > 0);
    }

    #[test]
    fn test_get_free_port_different_calls() {
        let port1 = get_free_port("127.0.0.1");
        let port2 = get_free_port("127.0.0.1");
        // Ports should be valid (>0). They will almost always differ, but
        // we just check they are valid.
        assert!(port1 > 0);
        assert!(port2 > 0);
    }

    // --- write_config ---

    #[test]
    fn test_write_config_default_separator() {
        let f = NamedTempFile::new().unwrap();
        let mut bag = HashMap::new();
        bag.insert("KEY1".into(), "val1".into());
        bag.insert("KEY2".into(), "val2".into());
        write_config(f.path(), &bag, "=").unwrap();
        let content = fs::read_to_string(f.path()).unwrap();
        assert!(content.contains("KEY1=val1\n"));
        assert!(content.contains("KEY2=val2\n"));
    }

    #[test]
    fn test_write_config_custom_separator() {
        let f = NamedTempFile::new().unwrap();
        let mut bag = HashMap::new();
        bag.insert("KEY".into(), "val".into());
        write_config(f.path(), &bag, ": ").unwrap();
        let content = fs::read_to_string(f.path()).unwrap();
        assert!(content.contains("KEY: val\n"));
    }

    // --- found_app ---

    #[test]
    fn test_found_app_returns_true() {
        assert!(found_app("Python"));
    }

    // --- check_requirements ---

    #[test]
    fn test_check_requirements_existing() {
        // "sh" should exist on any unix system
        assert!(check_requirements(&["sh"]));
    }

    #[test]
    fn test_check_requirements_missing() {
        assert!(!check_requirements(&["nonexistent_binary_xyz"]));
    }

    // --- command_output ---

    #[test]
    fn test_command_output_success() {
        let output = command_output("echo hello");
        assert_eq!(output.trim(), "hello");
    }

    #[test]
    fn test_command_output_failure() {
        let output = command_output("nonexistent_command_xyz 2>/dev/null");
        // On failure of the command itself, shell still runs; but for truly
        // broken commands, we get empty or an error. The Python version
        // catches exceptions, so we test graceful handling.
        // The command will run via sh -c, so the shell will produce stderr
        // but stdout will be empty.
        assert!(output.is_empty() || output.contains("not found"));
    }

    // --- Environment variable validation ---

    #[test]
    fn test_parse_positive_int_valid() {
        assert_eq!(parse_positive_int("TEST", "100"), Ok(100));
        assert_eq!(parse_positive_int("TEST", "1"), Ok(1));
        assert_eq!(parse_positive_int("TEST", "999999"), Ok(999999));
    }

    #[test]
    fn test_parse_positive_int_invalid() {
        assert!(parse_positive_int("TEST", "abc").is_err());
        assert!(parse_positive_int("TEST", "-5").is_err());
        assert!(parse_positive_int("TEST", "0").is_err());
        assert!(parse_positive_int("TEST", "").is_err());
    }

    #[test]
    fn test_validate_node_version_valid() {
        assert!(validate_node_version("18.17.0").is_ok());
        assert!(validate_node_version("18").is_ok());
        assert!(validate_node_version("20.0.0").is_ok());
        assert!(validate_node_version("18.17").is_ok());
    }

    #[test]
    fn test_validate_node_version_invalid() {
        assert!(validate_node_version("").is_err());
        assert!(validate_node_version("abc").is_err());
        assert!(validate_node_version("18.17").is_ok()); // This is actually valid
    }

    #[test]
    fn test_validate_env_vars_deprecated_warnings() {
        let mut env = HashMap::new();
        env.insert("UWSGI_PROCESSES".to_string(), "4".to_string());
        env.insert("UWSGI_MAX_REQUESTS".to_string(), "1000".to_string());

        let warnings = validate_env_vars(&env);
        assert!(warnings.iter().any(|w| w.contains("UWSGI_PROCESSES")));
        assert!(warnings.iter().any(|w| w.contains("UWSGI_MAX_REQUESTS")));
    }

    #[test]
    fn test_validate_env_vars_node_version_warning() {
        let mut env = HashMap::new();
        env.insert("NODE_VERSION".to_string(), "invalid".to_string());

        let warnings = validate_env_vars(&env);
        assert!(warnings.iter().any(|w| w.contains("NODE_VERSION")));
    }

    #[test]
    fn test_validate_env_vars_clean() {
        let mut env = HashMap::new();
        env.insert("NGINX_SERVER_NAME".to_string(), "example.com".to_string());
        env.insert("BIND_ADDRESS".to_string(), "127.0.0.1".to_string());

        let warnings = validate_env_vars(&env);
        assert!(warnings.is_empty());
    }
}
