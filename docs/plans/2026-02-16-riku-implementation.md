# Riku Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Port piku.py to Rust as a single binary (`riku`) that is a drop-in replacement, eliminating Python and uWSGI dependencies.

**Architecture:** Single binary with CLI commands (clap) + supervisor daemon mode. Nginx configs via tera templates. Native process supervisor replaces uWSGI Emperor. All external tools (git, nginx, acme.sh, language toolchains) invoked via `std::process::Command`.

**Tech Stack:** Rust, clap, anyhow, tera, serde+toml, nix, notify, regex, colored

**Design doc:** `docs/plans/2026-02-16-riku-rust-port-design.md`
**Reference implementation:** `piku.py` (the Python original — always check this for exact behavior)

---

## Phase 1: Project Scaffolding & Core Utilities

### Task 1: Initialize Cargo project

**Files:**
- Create: `riku/Cargo.toml`
- Create: `riku/src/main.rs`

**Step 1: Create the project**

```bash
cd /home/rakib/Code/others/piku
cargo init riku
```

**Step 2: Set up Cargo.toml with dependencies**

Replace `riku/Cargo.toml` with:

```toml
[package]
name = "riku"
version = "0.1.0"
edition = "2021"
description = "The smallest PaaS you've ever seen — in Rust"

[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = "1"
tera = "1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
nix = { version = "0.29", features = ["signal", "process", "user", "fs"] }
notify = "7"
regex = "1"
colored = "2"
log = "0.4"
env_logger = "0.11"
```

**Step 3: Stub main.rs**

```rust
fn main() {
    println!("riku - the smallest PaaS you've ever seen");
}
```

**Step 4: Verify it compiles**

Run: `cd /home/rakib/Code/others/piku/riku && cargo build`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add riku/
git commit -m "feat: initialize riku Rust project with dependencies"
```

---

### Task 2: Path constants and config module

**Files:**
- Create: `riku/src/config.rs`
- Modify: `riku/src/main.rs`

Reference: `piku.py:41-67` for all path constants.

**Step 1: Write tests for path resolution**

Add to `riku/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_paths_use_home() {
        let paths = PikuPaths::new(None);
        let home = env::var("HOME").unwrap();
        assert_eq!(paths.piku_root.to_str().unwrap(), format!("{home}/.piku"));
        assert_eq!(paths.app_root.to_str().unwrap(), format!("{home}/.piku/apps"));
        assert_eq!(paths.git_root.to_str().unwrap(), format!("{home}/.piku/repos"));
    }

    #[test]
    fn test_custom_piku_root() {
        let paths = PikuPaths::new(Some("/tmp/test-piku".into()));
        assert_eq!(paths.piku_root.to_str().unwrap(), "/tmp/test-piku");
        assert_eq!(paths.app_root.to_str().unwrap(), "/tmp/test-piku/apps");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/rakib/Code/others/piku/riku && cargo test`
Expected: FAIL — `PikuPaths` not defined

**Step 3: Implement PikuPaths**

```rust
use std::path::PathBuf;
use std::env;

/// All piku directory paths, matching piku.py lines 41-67.
#[derive(Debug, Clone)]
pub struct PikuPaths {
    pub piku_root: PathBuf,
    pub app_root: PathBuf,
    pub data_root: PathBuf,
    pub env_root: PathBuf,
    pub git_root: PathBuf,
    pub log_root: PathBuf,
    pub nginx_root: PathBuf,
    pub cache_root: PathBuf,
    pub uwsgi_root: PathBuf,
    pub uwsgi_available: PathBuf,
    pub uwsgi_enabled: PathBuf,
    pub uwsgi_log_maxsize: &'static str,
    pub acme_root: PathBuf,
    pub acme_www: PathBuf,
    pub plugin_root: PathBuf,
}

impl PikuPaths {
    pub fn new(custom_root: Option<PathBuf>) -> Self {
        let piku_root = custom_root.unwrap_or_else(|| {
            let root = env::var("PIKU_ROOT").ok();
            match root {
                Some(r) => PathBuf::from(r),
                None => {
                    let home = env::var("HOME").expect("HOME not set");
                    PathBuf::from(home).join(".piku")
                }
            }
        });

        let uwsgi_root = piku_root.join("uwsgi");

        PikuPaths {
            app_root: piku_root.join("apps"),
            data_root: piku_root.join("data"),
            env_root: piku_root.join("envs"),
            git_root: piku_root.join("repos"),
            log_root: piku_root.join("logs"),
            nginx_root: piku_root.join("nginx"),
            cache_root: piku_root.join("cache"),
            uwsgi_available: uwsgi_root.join("available"),
            uwsgi_enabled: uwsgi_root.join("enabled"),
            uwsgi_log_maxsize: "1048576",
            acme_root: PathBuf::from(env::var("HOME").unwrap_or_default()).join(".acme.sh"),
            acme_www: piku_root.join("acme"),
            plugin_root: piku_root.join("plugins"),
            uwsgi_root,
            piku_root,
        }
    }
}
```

**Step 4: Wire into main.rs**

```rust
mod config;

fn main() {
    println!("riku - the smallest PaaS you've ever seen");
}
```

**Step 5: Run tests to verify they pass**

Run: `cd /home/rakib/Code/others/piku/riku && cargo test`
Expected: PASS

**Step 6: Commit**

```bash
git add riku/src/config.rs riku/src/main.rs
git commit -m "feat: add PikuPaths config with all directory constants"
```

---

### Task 3: Utility functions

**Files:**
- Create: `riku/src/util.rs`
- Modify: `riku/src/main.rs`

Reference: `piku.py:224-370` for all utility functions.

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_app_name_valid() {
        assert_eq!(sanitize_app_name("my-app"), "my-app");
        assert_eq!(sanitize_app_name("my_app.v2"), "my_app.v2");
    }

    #[test]
    fn test_sanitize_app_name_strips_invalid() {
        assert_eq!(sanitize_app_name("my app!@#"), "my app");
        assert_eq!(sanitize_app_name("/leading-slash"), "leading-slash");
    }

    #[test]
    fn test_get_boolean() {
        assert!(get_boolean("true"));
        assert!(get_boolean("1"));
        assert!(get_boolean("yes"));
        assert!(get_boolean("on"));
        assert!(get_boolean("enabled"));
        assert!(get_boolean("y"));
        assert!(get_boolean("True"));
        assert!(get_boolean("YES"));
        assert!(!get_boolean("false"));
        assert!(!get_boolean("0"));
        assert!(!get_boolean("no"));
        assert!(!get_boolean("random"));
    }

    #[test]
    fn test_expandvars_simple() {
        let mut env = std::collections::HashMap::new();
        env.insert("FOO".to_string(), "bar".to_string());
        env.insert("BAZ".to_string(), "quux".to_string());
        assert_eq!(expandvars("$FOO and ${BAZ}", &env, None), "bar and quux");
    }

    #[test]
    fn test_expandvars_missing_var_kept() {
        let env = std::collections::HashMap::new();
        assert_eq!(expandvars("$MISSING", &env, None), "$MISSING");
    }

    #[test]
    fn test_expandvars_with_default() {
        let env = std::collections::HashMap::new();
        assert_eq!(expandvars("$MISSING", &env, Some("")), "");
    }

    #[test]
    fn test_parse_procfile_basic() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Procfile");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "web: gunicorn app:app").unwrap();
        writeln!(f, "worker: python worker.py").unwrap();
        writeln!(f, "# comment").unwrap();
        writeln!(f, "").unwrap();
        let workers = parse_procfile(&path).unwrap();
        assert_eq!(workers.len(), 2);
        assert_eq!(workers["web"], "gunicorn app:app");
        assert_eq!(workers["worker"], "python worker.py");
    }

    #[test]
    fn test_parse_procfile_wsgi_trumps_web() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Procfile");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "wsgi: myapp:application").unwrap();
        writeln!(f, "web: gunicorn app:app").unwrap();
        let workers = parse_procfile(&path).unwrap();
        assert!(workers.contains_key("wsgi"));
        assert!(!workers.contains_key("web"));
    }

    #[test]
    fn test_parse_procfile_missing_file() {
        let result = parse_procfile(std::path::Path::new("/nonexistent/Procfile"));
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_settings() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ENV");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "FOO=bar").unwrap();
        writeln!(f, "BAZ=$FOO-quux").unwrap();
        writeln!(f, "# comment").unwrap();
        let env = parse_settings(&path, &mut std::collections::HashMap::new()).unwrap();
        assert_eq!(env["FOO"], "bar");
        assert_eq!(env["BAZ"], "bar-quux");
    }

    #[test]
    fn test_get_free_port() {
        let port = get_free_port("");
        assert!(port > 0);
        assert!(port < 65536);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/rakib/Code/others/piku/riku && cargo test`
Expected: FAIL

**Step 3: Add tempfile dev-dependency**

Add to `riku/Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
```

**Step 4: Implement util.rs**

```rust
use std::collections::HashMap;
use std::fs;
use std::net::TcpListener;
use std::path::Path;
use anyhow::{Result, bail};
use colored::Colorize;
use regex::Regex;

/// Sanitize the app name — only allow alphanumeric, dots, underscores, hyphens.
/// Strip leading slashes. Matches piku.py:224-228.
pub fn sanitize_app_name(app: &str) -> String {
    let cleaned: String = app
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-' || *c == ' ')
        .collect();
    cleaned.trim_start_matches('/').trim_end().to_string()
}

/// Check app exists, return sanitized name or exit. Matches piku.py:231-238.
pub fn exit_if_invalid(app: &str, app_root: &Path) -> Result<String> {
    let app = sanitize_app_name(app);
    if !app_root.join(&app).exists() {
        eprintln!("{}", format!("Error: app '{}' not found.", app).red());
        std::process::exit(1);
    }
    Ok(app)
}

/// Find a free TCP port. Matches piku.py:241-248.
pub fn get_free_port(address: &str) -> u16 {
    let addr = if address.is_empty() { "0.0.0.0" } else { address };
    let listener = TcpListener::bind(format!("{}:0", addr)).expect("Failed to bind to address");
    listener.local_addr().unwrap().port()
}

/// Convert a boolean-ish string to a boolean. Matches piku.py:251-254.
pub fn get_boolean(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "1" | "on" | "true" | "enabled" | "yes" | "y")
}

/// Write a config file from key-value pairs. Matches piku.py:257-263.
pub fn write_config(filename: &Path, bag: &HashMap<String, String>, separator: &str) -> Result<()> {
    let mut content = String::new();
    for (k, v) in bag {
        content.push_str(&format!("{}{}{}\n", k, separator, v));
    }
    fs::write(filename, content)?;
    Ok(())
}

/// Expand shell-style environment variables. Matches piku.py:317-324.
pub fn expandvars(buffer: &str, env: &HashMap<String, String>, default: Option<&str>) -> String {
    let re = Regex::new(r"\$(\w+|\{([^}]*)\})").unwrap();
    re.replace_all(buffer, |caps: &regex::Captures| {
        let var_name = caps.get(2)
            .or_else(|| caps.get(1))
            .map(|m| m.as_str())
            .unwrap_or("");
        // Strip braces if present
        let var_name = var_name.trim_start_matches('{').trim_end_matches('}');
        match env.get(var_name) {
            Some(val) => val.clone(),
            None => match default {
                Some(d) => d.to_string(),
                None => caps.get(0).unwrap().as_str().to_string(),
            },
        }
    })
    .to_string()
}

/// Execute a command and return its output. Matches piku.py:327-333.
pub fn command_output(cmd: &str) -> String {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

/// Parse a Procfile. Returns None if file doesn't exist, Some(map) otherwise.
/// Matches piku.py:279-314.
pub fn parse_procfile(filename: &Path) -> Option<HashMap<String, String>> {
    if !filename.exists() {
        return None;
    }

    let content = fs::read_to_string(filename).ok()?;
    let cron_re = Regex::new(
        r"^((?:\*|[0-9]+)(?:/[0-9]+)?)\s+((?:\*|[0-9]+)(?:/[0-9]+)?)\s+((?:\*|[0-9]+)(?:/[0-9]+)?)\s+((?:\*|[0-9]+)(?:/[0-9]+)?)\s+((?:\*|[0-9]+)(?:/[0-9]+)?)\s+(.*)$"
    ).unwrap();
    let limits = [59, 24, 31, 12, 7];

    let mut workers = HashMap::new();

    for (line_number, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if let Some((kind, command)) = line.split_once(':') {
            let kind = kind.trim().to_string();
            let command = command.trim().to_string();

            if kind.starts_with("cron") {
                if let Some(caps) = cron_re.captures(&command) {
                    let mut valid = true;
                    for i in 0..limits.len() {
                        let val_str = caps.get(i + 1).unwrap().as_str();
                        let val_str = val_str.replace("*/", "").replace("*", "1");
                        if let Ok(val) = val_str.parse::<u32>() {
                            if val > limits[i] {
                                valid = false;
                                break;
                            }
                        }
                    }
                    if !valid {
                        eprintln!("{}", format!(
                            "Warning: misformatted Procfile entry '{}' at line {}", line, line_number
                        ).yellow());
                        continue;
                    }
                }
            }

            if workers.contains_key(&kind) {
                eprintln!("{}", format!(
                    "Warning: found multiple {} workers, only the last one will be used.", kind
                ).yellow());
            }
            workers.insert(kind, command);
        } else {
            eprintln!("{}", format!(
                "Warning: misformatted Procfile entry '{}' at line {}", line, line_number
            ).yellow());
        }
    }

    // WSGI trumps regular web workers
    if workers.contains_key("wsgi") || workers.contains_key("jwsgi") || workers.contains_key("rwsgi") {
        if workers.contains_key("web") {
            eprintln!("{}", "Warning: found both 'wsgi' and 'web' workers, disabling 'web'".yellow());
            workers.remove("web");
        }
    }

    Some(workers)
}

/// Parse a settings/ENV file with variable interpolation. Matches piku.py:336-352.
pub fn parse_settings(filename: &Path, env: &mut HashMap<String, String>) -> Result<HashMap<String, String>> {
    if !filename.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(filename)?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim().to_string();
            let v = v.trim().to_string();
            let expanded = expandvars(&v, env, None);
            env.insert(k, expanded);
        } else {
            eprintln!("{}", format!("Error: malformed setting '{}', ignoring file.", line).red());
            return Ok(HashMap::new());
        }
    }

    Ok(env.clone())
}

/// Check if all required binaries exist. Matches piku.py:355-364.
pub fn check_requirements(binaries: &[&str]) -> bool {
    println!("{}", format!("-----> Checking requirements: {:?}", binaries).green());
    let results: Vec<Option<std::path::PathBuf>> = binaries
        .iter()
        .map(|b| which::which(b).ok())
        .collect();
    println!("{:?}", results);
    !results.iter().any(|r| r.is_none())
}

/// Helper to print app detected message. Matches piku.py:367-370.
pub fn found_app(kind: &str) -> bool {
    println!("{}", format!("-----> {} app detected.", kind).green());
    true
}

/// Print colored output matching piku's echo(fg=...) style.
pub fn echo(msg: &str, color: &str) {
    match color {
        "green" => println!("{}", msg.green()),
        "yellow" => println!("{}", msg.yellow()),
        "red" => eprintln!("{}", msg.red()),
        "white" | _ => println!("{}", msg),
    }
}

/// Set up authorized_keys for SSH. Matches piku.py:266-276.
pub fn setup_authorized_keys(ssh_fingerprint: &str, script_path: &str, pubkey: &str) -> Result<()> {
    let home = std::env::var("HOME")?;
    let ssh_dir = Path::new(&home).join(".ssh");
    let authorized_keys = ssh_dir.join("authorized_keys");

    if !ssh_dir.exists() {
        fs::create_dir_all(&ssh_dir)?;
    }

    let entry = format!(
        "command=\"FINGERPRINT={} NAME=default {} $SSH_ORIGINAL_COMMAND\",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding {}\n",
        ssh_fingerprint, script_path, pubkey
    );

    let mut content = if authorized_keys.exists() {
        fs::read_to_string(&authorized_keys)?
    } else {
        String::new()
    };
    content.push_str(&entry);
    fs::write(&authorized_keys, content)?;

    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&authorized_keys, fs::Permissions::from_mode(0o600))?;

    Ok(())
}
```

**Step 5: Add `which` dependency to Cargo.toml**

```toml
which = "7"
```

**Step 6: Wire into main.rs**

Add `mod util;` to `main.rs`.

**Step 7: Run tests**

Run: `cd /home/rakib/Code/others/piku/riku && cargo test`
Expected: All PASS

**Step 8: Commit**

```bash
git add riku/
git commit -m "feat: add core utility functions (sanitize, parse, expandvars)"
```

---

## Phase 2: CLI Framework

### Task 4: Clap CLI skeleton with all subcommands

**Files:**
- Create: `riku/src/cli/mod.rs`
- Create: `riku/src/cli/apps.rs`
- Create: `riku/src/cli/git.rs`
- Create: `riku/src/cli/setup.rs`
- Create: `riku/src/cli/scp.rs`
- Modify: `riku/src/main.rs`

Reference: `piku.py:1381-1822` for all CLI commands.

**Step 1: Define the clap CLI structure in `cli/mod.rs`**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "riku", about = "The smallest PaaS you've ever seen")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List apps
    Apps,

    /// Show/manage config
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Force deploy an app
    Deploy {
        app: String,
    },

    /// Destroy an app
    Destroy {
        app: String,
    },

    /// Tail running logs
    Logs {
        app: String,
        #[arg(default_value = "*")]
        process: String,
    },

    /// Show/manage processes
    #[command(subcommand)]
    Ps(PsCommands),

    /// Run a command in app context
    Run {
        app: String,
        #[arg(trailing_var_arg = true)]
        cmd: Vec<String>,
    },

    /// Restart an app
    Restart {
        app: String,
    },

    /// Stop an app
    Stop {
        app: String,
    },

    /// Initialize environment
    #[command(subcommand)]
    Setup(SetupCommands),

    /// Self-update riku
    Update,

    /// Start the process supervisor daemon
    Supervisor,

    /// Display help
    Help,

    // --- Internal commands ---
    /// INTERNAL: Post-receive git hook
    #[command(name = "git-hook", hide = true)]
    GitHook {
        app: String,
    },

    /// INTERNAL: Handle git push
    #[command(name = "git-receive-pack", hide = true)]
    GitReceivePack {
        app: String,
    },

    /// INTERNAL: Handle git fetch
    #[command(name = "git-upload-pack", hide = true)]
    GitUploadPack {
        app: String,
    },

    /// SCP wrapper
    #[command(hide = true, trailing_var_arg = true)]
    Scp {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show config for an app (alias: config:get with no key shows all)
    Show { app: String },
    /// Get a single config value
    Get { app: String, setting: String },
    /// Set config values
    Set {
        app: String,
        #[arg(trailing_var_arg = true)]
        settings: Vec<String>,
    },
    /// Unset config values
    Unset {
        app: String,
        #[arg(trailing_var_arg = true)]
        settings: Vec<String>,
    },
    /// Show live running config
    Live { app: String },
}

#[derive(Subcommand)]
pub enum PsCommands {
    /// Show process count
    Show { app: String },
    /// Scale workers
    Scale {
        app: String,
        #[arg(trailing_var_arg = true)]
        settings: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum SetupCommands {
    /// Initialize piku environment
    Init,
    /// Add SSH key
    Ssh { public_key_file: String },
}
```

**Step 2: Stub command handlers in `cli/apps.rs`**

```rust
use anyhow::Result;
use crate::config::PikuPaths;

pub fn cmd_apps(paths: &PikuPaths) -> Result<()> {
    println!("TODO: list apps");
    Ok(())
}

pub fn cmd_config_show(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: show config for {}", app);
    Ok(())
}

pub fn cmd_config_get(paths: &PikuPaths, app: &str, setting: &str) -> Result<()> {
    println!("TODO: get {} for {}", setting, app);
    Ok(())
}

pub fn cmd_config_set(paths: &PikuPaths, app: &str, settings: &[String]) -> Result<()> {
    println!("TODO: set config for {}", app);
    Ok(())
}

pub fn cmd_config_unset(paths: &PikuPaths, app: &str, settings: &[String]) -> Result<()> {
    println!("TODO: unset config for {}", app);
    Ok(())
}

pub fn cmd_config_live(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: show live config for {}", app);
    Ok(())
}

pub fn cmd_deploy(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: deploy {}", app);
    Ok(())
}

pub fn cmd_destroy(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: destroy {}", app);
    Ok(())
}

pub fn cmd_logs(paths: &PikuPaths, app: &str, process: &str) -> Result<()> {
    println!("TODO: logs for {}", app);
    Ok(())
}

pub fn cmd_ps_show(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: ps for {}", app);
    Ok(())
}

pub fn cmd_ps_scale(paths: &PikuPaths, app: &str, settings: &[String]) -> Result<()> {
    println!("TODO: scale for {}", app);
    Ok(())
}

pub fn cmd_run(paths: &PikuPaths, app: &str, cmd: &[String]) -> Result<()> {
    println!("TODO: run for {}", app);
    Ok(())
}

pub fn cmd_restart(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: restart {}", app);
    Ok(())
}

pub fn cmd_stop(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: stop {}", app);
    Ok(())
}

pub fn cmd_update() -> Result<()> {
    println!("TODO: update");
    Ok(())
}
```

**Step 3: Stub `cli/git.rs`**

```rust
use anyhow::Result;
use crate::config::PikuPaths;

pub fn cmd_git_hook(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: git hook for {}", app);
    Ok(())
}

pub fn cmd_git_receive_pack(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: git-receive-pack for {}", app);
    Ok(())
}

pub fn cmd_git_upload_pack(paths: &PikuPaths, app: &str) -> Result<()> {
    println!("TODO: git-upload-pack for {}", app);
    Ok(())
}
```

**Step 4: Stub `cli/setup.rs`**

```rust
use anyhow::Result;
use crate::config::PikuPaths;

pub fn cmd_setup(paths: &PikuPaths) -> Result<()> {
    println!("TODO: setup");
    Ok(())
}

pub fn cmd_setup_ssh(paths: &PikuPaths, public_key_file: &str) -> Result<()> {
    println!("TODO: setup:ssh");
    Ok(())
}
```

**Step 5: Stub `cli/scp.rs`**

```rust
use anyhow::Result;
use crate::config::PikuPaths;

pub fn cmd_scp(paths: &PikuPaths, args: &[String]) -> Result<()> {
    println!("TODO: scp");
    Ok(())
}
```

**Step 6: Wire main.rs to dispatch commands**

```rust
mod cli;
mod config;
mod util;

use clap::Parser;
use cli::{Cli, Commands, ConfigCommands, PsCommands, SetupCommands};
use config::PikuPaths;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let paths = PikuPaths::new(None);

    match cli.command {
        Commands::Apps => cli::apps::cmd_apps(&paths),
        Commands::Config(sub) => match sub {
            ConfigCommands::Show { app } => cli::apps::cmd_config_show(&paths, &app),
            ConfigCommands::Get { app, setting } => cli::apps::cmd_config_get(&paths, &app, &setting),
            ConfigCommands::Set { app, settings } => cli::apps::cmd_config_set(&paths, &app, &settings),
            ConfigCommands::Unset { app, settings } => cli::apps::cmd_config_unset(&paths, &app, &settings),
            ConfigCommands::Live { app } => cli::apps::cmd_config_live(&paths, &app),
        },
        Commands::Deploy { app } => cli::apps::cmd_deploy(&paths, &app),
        Commands::Destroy { app } => cli::apps::cmd_destroy(&paths, &app),
        Commands::Logs { app, process } => cli::apps::cmd_logs(&paths, &app, &process),
        Commands::Ps(sub) => match sub {
            PsCommands::Show { app } => cli::apps::cmd_ps_show(&paths, &app),
            PsCommands::Scale { app, settings } => cli::apps::cmd_ps_scale(&paths, &app, &settings),
        },
        Commands::Run { app, cmd } => cli::apps::cmd_run(&paths, &app, &cmd),
        Commands::Restart { app } => cli::apps::cmd_restart(&paths, &app),
        Commands::Stop { app } => cli::apps::cmd_stop(&paths, &app),
        Commands::Setup(sub) => match sub {
            SetupCommands::Init => cli::setup::cmd_setup(&paths),
            SetupCommands::Ssh { public_key_file } => cli::setup::cmd_setup_ssh(&paths, &public_key_file),
        },
        Commands::Update => cli::apps::cmd_update(),
        Commands::Supervisor => {
            println!("TODO: supervisor");
            Ok(())
        },
        Commands::Help => {
            // clap handles --help automatically; this is for `riku help`
            use clap::CommandFactory;
            Cli::command().print_help()?;
            Ok(())
        },
        Commands::GitHook { app } => cli::git::cmd_git_hook(&paths, &app),
        Commands::GitReceivePack { app } => cli::git::cmd_git_receive_pack(&paths, &app),
        Commands::GitUploadPack { app } => cli::git::cmd_git_upload_pack(&paths, &app),
        Commands::Scp { args } => cli::scp::cmd_scp(&paths, &args),
    }
}
```

**Step 7: Verify it compiles and help works**

Run: `cd /home/rakib/Code/others/piku/riku && cargo run -- --help`
Expected: Shows usage with all subcommands

**Step 8: Commit**

```bash
git add riku/
git commit -m "feat: add clap CLI skeleton with all subcommands"
```

---

### Task 5: Implement CLI commands (apps, config, logs, ps, run, stop, restart, destroy)

**Files:**
- Modify: `riku/src/cli/apps.rs`

Reference: `piku.py:1402-1634` for all command implementations.

**Step 1: Implement all commands**

Each command follows the same pattern as the Python version — read files from `paths`, parse settings, print output. The implementations are straightforward file I/O with colored output:

- `cmd_apps`: List dirs in `app_root`, check for enabled configs
- `cmd_config_show`: Read `ENV_ROOT/app/ENV`
- `cmd_config_get`: Parse settings, print specific key
- `cmd_config_set`: Parse settings, update, write, call `do_deploy`
- `cmd_config_unset`: Parse settings, remove keys, write, call `do_deploy`
- `cmd_config_live`: Read `ENV_ROOT/app/LIVE_ENV`
- `cmd_deploy`: Call `do_deploy(app)`
- `cmd_destroy`: Remove app/git/env/log dirs, keep data/cache (match piku.py:1510-1549)
- `cmd_logs`: Glob log files, call `multi_tail` equivalent
- `cmd_ps_show`: Read `ENV_ROOT/app/SCALING`
- `cmd_ps_scale`: Parse scaling, compute deltas, call `do_deploy`
- `cmd_run`: Load LIVE_ENV, spawn command with env
- `cmd_restart`: Stop + spawn_app
- `cmd_stop`: Remove enabled configs

Implement each one matching the Python behavior exactly. Use `util::exit_if_invalid`, `util::parse_settings`, `util::write_config`, `util::echo`.

For `cmd_logs`, implement a `multi_tail` function that:
- Opens log files, seeks to end
- Catches up last 20 lines
- Polls every 1s for new content
- Tracks inodes for log rotation

**Step 2: Run `cargo build` to verify compilation**

**Step 3: Commit**

```bash
git add riku/src/cli/apps.rs
git commit -m "feat: implement all user-facing CLI commands"
```

---

### Task 6: Implement setup and git commands

**Files:**
- Modify: `riku/src/cli/setup.rs`
- Modify: `riku/src/cli/git.rs`
- Modify: `riku/src/cli/scp.rs`

Reference: `piku.py:1636-1773` for setup/git commands.

**Step 1: Implement `cmd_setup`**

Match `piku.py:1636-1670`:
- Create all required directories
- Write supervisor config (TOML instead of uWSGI emperor INI)
- Mark the script as executable

**Step 2: Implement `cmd_setup_ssh`**

Match `piku.py:1673-1696`:
- Read public key from file or stdin (`-`)
- Extract fingerprint via `ssh-keygen -lf`
- Call `setup_authorized_keys()`

**Step 3: Implement `cmd_git_receive_pack`**

Match `piku.py:1733-1754`:
- Create bare git repo if needed (`git init --bare`)
- Write post-receive hook that calls `riku git-hook <app>`
- Call `git-shell` to handle the actual push

**Step 4: Implement `cmd_git_hook`**

Match `piku.py:1709-1730`:
- Read stdin for `oldrev newrev refname`
- Create app dir if needed
- Clone from bare repo
- Call `do_deploy(app, newrev)`

**Step 5: Implement `cmd_git_upload_pack` and `cmd_scp`**

Match `piku.py:1757-1772` — simple shell-outs.

**Step 6: Implement `cmd_update`**

Match `piku.py:1800-1815`:
- Download latest binary from release URL via `curl`
- Replace current binary

**Step 7: Verify compilation**

Run: `cd /home/rakib/Code/others/piku/riku && cargo build`

**Step 8: Commit**

```bash
git add riku/src/cli/
git commit -m "feat: implement setup, git, and scp commands"
```

---

## Phase 3: Deploy Pipeline

### Task 7: Deploy orchestration (`do_deploy`)

**Files:**
- Create: `riku/src/deploy/mod.rs`
- Modify: `riku/src/main.rs`

Reference: `piku.py:373-449` for `do_deploy()`.

**Step 1: Write test for runtime detection**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_python_runtime() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("requirements.txt"), "flask").unwrap();
        let rt = detect_runtime(dir.path());
        assert_eq!(rt, Some(Runtime::Python));
    }

    #[test]
    fn test_detect_node_runtime() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        let rt = detect_runtime(dir.path());
        assert_eq!(rt, Some(Runtime::Node));
    }

    #[test]
    fn test_detect_no_runtime() {
        let dir = tempdir().unwrap();
        let rt = detect_runtime(dir.path());
        assert_eq!(rt, None);
    }
}
```

**Step 2: Implement runtime detection enum and `detect_runtime()`**

```rust
#[derive(Debug, PartialEq)]
pub enum Runtime {
    Python,
    PythonPoetry,
    PythonUv,
    Node,
    Ruby,
    Go,
    Rust,
    JavaMaven,
    JavaGradle,
    ClojureCli,
    ClojureLein,
    Identity,
}

pub fn detect_runtime(app_path: &Path) -> Option<Runtime> {
    // Check in same order as piku.py:410-445
    if app_path.join("requirements.txt").exists() {
        Some(Runtime::Python)
    } else if app_path.join("pyproject.toml").exists() {
        if command_output("which poetry").trim().len() > 0 {
            Some(Runtime::PythonPoetry)
        } else if command_output("which uv").trim().len() > 0 {
            Some(Runtime::PythonUv)
        } else {
            Some(Runtime::Python)
        }
    } else if app_path.join("Gemfile").exists() {
        Some(Runtime::Ruby)
    } else if app_path.join("package.json").exists() {
        Some(Runtime::Node)
    } else if app_path.join("pom.xml").exists() {
        Some(Runtime::JavaMaven)
    } else if app_path.join("build.gradle").exists() {
        Some(Runtime::JavaGradle)
    } else if app_path.join("Godeps").exists()
        || app_path.join("go.mod").exists()
        || glob_exists(app_path, "*.go")
    {
        Some(Runtime::Go)
    } else if app_path.join("deps.edn").exists() {
        Some(Runtime::ClojureCli)
    } else if app_path.join("project.clj").exists() {
        Some(Runtime::ClojureLein)
    } else if app_path.join("Cargo.toml").exists() {
        Some(Runtime::Rust)
    } else {
        None
    }
}
```

**Step 3: Implement `do_deploy()`**

Orchestration function matching `piku.py:373-449`:
1. Git fetch/reset/submodule
2. Parse Procfile
3. Run preflight if exists
4. Detect runtime → call deployer
5. Run release if exists
6. Call `spawn_app()`

**Step 4: Run tests**

Run: `cd /home/rakib/Code/others/piku/riku && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add riku/src/deploy/
git commit -m "feat: add deploy orchestration with runtime detection"
```

---

### Task 8: Runtime deployers (all languages)

**Files:**
- Create: `riku/src/deploy/python.rs`
- Create: `riku/src/deploy/node.rs`
- Create: `riku/src/deploy/ruby.rs`
- Create: `riku/src/deploy/go.rs`
- Create: `riku/src/deploy/rust.rs`
- Create: `riku/src/deploy/java.rs`
- Create: `riku/src/deploy/clojure.rs`
- Create: `riku/src/deploy/identity.rs`

Reference: `piku.py:452-788` for all deployers.

Each deployer is a function `deploy_<runtime>(app: &str, paths: &PikuPaths) -> Result<()>` that:
1. Checks binary requirements
2. Creates/updates environment
3. Installs dependencies

All use `std::process::Command` to shell out to language toolchains.

**Step 1: Implement `deploy_python()`** — match `piku.py:452-500`
- Create virtualenv if not exists
- Run `pip install -r requirements.txt`

**Step 2: Implement `deploy_python_poetry()`** — match `piku.py:503-540`
**Step 3: Implement `deploy_python_uv()`** — match `piku.py:543-575`
**Step 4: Implement `deploy_node()`** — match `piku.py:578-640`
**Step 5: Implement `deploy_ruby()`** — match `piku.py:643-670`
**Step 6: Implement `deploy_go()`** — match `piku.py:673-710`
**Step 7: Implement `deploy_rust()`** — match `piku.py:713-740`
**Step 8: Implement `deploy_java_maven()` and `deploy_java_gradle()`** — match `piku.py:743-770`
**Step 9: Implement `deploy_clojure_cli()` and `deploy_clojure_lein()`** — match `piku.py:773-788`
**Step 10: Implement `deploy_identity()`** — no-op deployer

**Step 11: Verify compilation**

Run: `cd /home/rakib/Code/others/piku/riku && cargo build`

**Step 12: Commit**

```bash
git add riku/src/deploy/
git commit -m "feat: implement all runtime deployers"
```

---

## Phase 4: Worker Spawning & Nginx

### Task 9: Worker spawning (`spawn_app` + `spawn_worker`)

**Files:**
- Create a `spawn_app()` function in `riku/src/deploy/mod.rs` (or a new file `riku/src/deploy/spawn.rs`)

Reference: `piku.py:790-1308` for `spawn_app()` and `spawn_worker()`.

**Step 1: Write test for worker TOML config generation**

```rust
#[test]
fn test_spawn_worker_generates_toml() {
    // Test that spawn_worker creates a valid TOML file
    // with correct [worker], [env], [options] sections
}
```

**Step 2: Implement `spawn_app()`**

Match `piku.py:790-1134`:
- Parse Procfile, load ENV, set up environment
- Handle port assignment
- Call nginx config generation if web worker
- For each worker type, call `spawn_worker()`
- Write SCALING and LIVE_ENV files

**Step 3: Implement `spawn_worker()`**

Match `piku.py:1137-1308` but output TOML instead of INI:
- Generate worker TOML config with all settings
- Write to `uwsgi-available/` directory
- Copy (or symlink) to `uwsgi-enabled/`

Key difference: No uWSGI-specific settings (plugin, module, etc.). Instead:
- `web` workers: command is run directly (e.g., `gunicorn ...`)
- `wsgi` workers: translated to `gunicorn <module> --bind <addr>` command
- `static` workers: no process needed, nginx handles it
- `cron` workers: stored with cron expression, supervisor handles scheduling
- Generic workers: command is run directly as attached daemon

**Step 4: Run tests**

**Step 5: Commit**

```bash
git add riku/src/deploy/
git commit -m "feat: implement spawn_app and spawn_worker with TOML configs"
```

---

### Task 10: Nginx config generation

**Files:**
- Create: `riku/src/nginx.rs`
- Create: `riku/templates/nginx.conf.tera`
- Create: `riku/templates/nginx_https_only.conf.tera`
- Create: `riku/templates/nginx_common.conf.tera`
- Create: `riku/templates/nginx_portmap.conf.tera`
- Create: `riku/templates/nginx_acme_firstrun.conf.tera`
- Create: `riku/templates/nginx_static.conf.tera`
- Create: `riku/templates/nginx_cache.conf.tera`
- Create: `riku/templates/nginx_uwsgi.conf.tera`

Reference: `piku.py:69-217` for all nginx templates.

**Step 1: Convert Python string templates to Tera templates**

Take each `NGINX_*` constant from `piku.py:69-217` and convert to a `.tera` file. Replace Python `{variable}` syntax with Tera `{{ variable }}` syntax.

**Step 2: Write test for template rendering**

```rust
#[test]
fn test_nginx_config_renders() {
    let mut ctx = tera::Context::new();
    ctx.insert("APP", "myapp");
    ctx.insert("NGINX_SERVER_NAME", "example.com");
    ctx.insert("BIND_ADDRESS", "127.0.0.1");
    ctx.insert("PORT", "5000");
    // ... etc
    let result = render_nginx_config(&ctx, false);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(config.contains("server_name example.com"));
}
```

**Step 3: Implement `nginx.rs`**

Functions:
- `render_nginx_config(ctx, https_only) -> Result<String>` — render full nginx config
- `setup_nginx(app, env, paths) -> Result<()>` — write config, provision SSL, validate
- `provision_ssl(app, domain, paths) -> Result<()>` — try acme.sh, fallback to self-signed

Templates embedded via:
```rust
lazy_static! {
    static ref TEMPLATES: Tera = {
        let mut tera = Tera::default();
        tera.add_raw_template("nginx.conf", include_str!("../templates/nginx.conf.tera")).unwrap();
        // ... all templates
        tera
    };
}
```

Note: Use `once_cell::sync::Lazy` or `std::sync::LazyLock` (stable in Rust 1.80+) instead of `lazy_static`.

**Step 4: Run tests**

**Step 5: Commit**

```bash
git add riku/src/nginx.rs riku/templates/
git commit -m "feat: add nginx config generation with tera templates"
```

---

## Phase 5: Process Supervisor

### Task 11: Worker config parsing

**Files:**
- Create: `riku/src/supervisor/mod.rs`
- Create: `riku/src/supervisor/config.rs`

**Step 1: Write test for TOML config parsing**

```rust
#[test]
fn test_parse_worker_config() {
    let toml_str = r#"
    [worker]
    app = "myapp"
    kind = "web"
    command = "gunicorn app:app"
    ordinal = 1

    [env]
    PORT = "5000"

    [options]
    working_dir = "/home/piku/.piku/apps/myapp"
    log_file = "/home/piku/.piku/logs/myapp/web.1.log"
    uid = "piku"
    gid = "piku"
    "#;

    let config: WorkerConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.worker.app, "myapp");
    assert_eq!(config.worker.kind, "web");
    assert_eq!(config.env["PORT"], "5000");
}
```

**Step 2: Implement `WorkerConfig` struct**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub worker: WorkerInfo,
    pub env: HashMap<String, String>,
    pub options: WorkerOptions,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub app: String,
    pub kind: String,
    pub command: String,
    pub ordinal: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerOptions {
    pub working_dir: String,
    pub log_file: String,
    pub uid: String,
    pub gid: String,
}
```

**Step 3: Run tests, commit**

```bash
git add riku/src/supervisor/
git commit -m "feat: add worker TOML config parsing"
```

---

### Task 12: Cron expression parser

**Files:**
- Create: `riku/src/supervisor/cron.rs`

**Step 1: Write tests**

```rust
#[test]
fn test_cron_next_fire() {
    // "*/5 * * * *" = every 5 minutes
    let expr = CronExpr::parse("*/5 * * * *").unwrap();
    // next_fire should return a time within the next 5 minutes
    let next = expr.next_fire_after(chrono::Utc::now());
    assert!(next > chrono::Utc::now());
}

#[test]
fn test_cron_parse_invalid() {
    assert!(CronExpr::parse("invalid").is_err());
}
```

Note: Add `chrono` to dependencies if needed for time handling, or use `std::time` with manual calculations.

**Step 2: Implement `CronExpr`**

A simple cron parser that handles: `minute hour day month weekday`. Supports `*`, `*/N`, and literal numbers. No need for a full cron library — the Python version only supports basic patterns.

**Step 3: Run tests, commit**

```bash
git add riku/src/supervisor/cron.rs
git commit -m "feat: add cron expression parser"
```

---

### Task 13: Process manager

**Files:**
- Create: `riku/src/supervisor/process.rs`

**Step 1: Implement `ManagedProcess` struct**

```rust
pub struct ManagedProcess {
    pub config: WorkerConfig,
    pub child: Option<std::process::Child>,
    pub started_at: std::time::Instant,
    pub restart_count: u32,
    pub backoff_secs: u64,
}
```

**Step 2: Implement process lifecycle methods**

- `start()` — spawn child process with env, working_dir, log redirection
- `stop()` — send SIGTERM, wait 10s grace, SIGKILL if still alive
- `is_running()` — check if child process is still alive
- `restart_with_backoff()` — exponential backoff: 1s, 2s, 4s... cap at 60s, reset after 60s stable

**Step 3: Write tests for backoff logic**

```rust
#[test]
fn test_backoff_calculation() {
    assert_eq!(calculate_backoff(0), 1);
    assert_eq!(calculate_backoff(1), 2);
    assert_eq!(calculate_backoff(2), 4);
    assert_eq!(calculate_backoff(6), 60); // capped at 60
    assert_eq!(calculate_backoff(10), 60); // still capped
}
```

**Step 4: Run tests, commit**

```bash
git add riku/src/supervisor/process.rs
git commit -m "feat: add process manager with crash recovery and backoff"
```

---

### Task 14: Supervisor daemon main loop

**Files:**
- Modify: `riku/src/supervisor/mod.rs`
- Modify: `riku/src/main.rs` (wire `Commands::Supervisor`)

**Step 1: Implement the supervisor main loop**

```rust
pub fn run_supervisor(paths: &PikuPaths) -> Result<()> {
    // 1. Scan uwsgi-enabled/ for existing .toml configs
    // 2. Start all found workers
    // 3. Set up filesystem watcher on uwsgi-enabled/
    // 4. Set up signal handlers (SIGTERM, SIGINT, SIGHUP)
    // 5. Main loop:
    //    a. Check for filesystem events (new/modified/removed configs)
    //    b. Check for crashed processes, restart with backoff
    //    c. Check cron schedules, fire due jobs
    //    d. Sleep 1s
}
```

**Step 2: Implement signal handling**

Use `nix::sys::signal` for SIGTERM/SIGINT/SIGHUP handling. On SIGTERM/SIGINT → graceful shutdown. On SIGHUP → rescan configs.

**Step 3: Wire `Commands::Supervisor` in main.rs**

```rust
Commands::Supervisor => supervisor::run_supervisor(&paths),
```

**Step 4: Verify compilation**

Run: `cd /home/rakib/Code/others/piku/riku && cargo build`

**Step 5: Commit**

```bash
git add riku/src/supervisor/ riku/src/main.rs
git commit -m "feat: implement supervisor daemon with fs watching and signal handling"
```

---

## Phase 6: Plugin System & Polish

### Task 15: Shell-based plugin system

**Files:**
- Create: `riku/src/plugins.rs`
- Modify: `riku/src/main.rs`

Reference: `piku.py:1775-1822` for plugin loading.

**Step 1: Implement plugin discovery**

```rust
/// Scan plugin_root for executable files/directories.
/// Each becomes an external subcommand.
pub fn discover_plugins(plugin_root: &Path) -> Vec<String> {
    // List items in plugin_root
    // Filter to executable files
    // Return their names
}
```

**Step 2: Handle unknown subcommands as plugins in main.rs**

Use clap's `allow_external_subcommands` or handle the unrecognized command case by looking for a plugin with that name and executing it.

**Step 3: Commit**

```bash
git add riku/src/plugins.rs riku/src/main.rs
git commit -m "feat: add shell-based plugin system"
```

---

### Task 16: Integration testing

**Files:**
- Create: `riku/tests/integration_test.rs`

**Step 1: Write integration test for full deploy flow**

```rust
#[test]
fn test_setup_creates_directories() {
    // Create temp PIKU_ROOT
    // Run riku setup
    // Verify all directories exist
}

#[test]
fn test_app_lifecycle() {
    // Setup temp environment
    // Create a bare git repo
    // Simulate git-hook with a simple app
    // Verify app directory created
    // Verify worker configs generated
    // Run riku stop
    // Verify configs removed
}
```

**Step 2: Run tests**

Run: `cd /home/rakib/Code/others/piku/riku && cargo test`

**Step 3: Commit**

```bash
git add riku/tests/
git commit -m "test: add integration tests for app lifecycle"
```

---

### Task 17: Final polish and release build

**Files:**
- Modify: `riku/Cargo.toml` (release profile)
- Modify: `CLAUDE.md` (update if needed)

**Step 1: Add release profile optimizations**

```toml
[profile.release]
lto = true
strip = true
codegen-units = 1
```

**Step 2: Build release binary**

Run: `cd /home/rakib/Code/others/piku/riku && cargo build --release`

**Step 3: Verify binary size and test**

Run: `ls -lh target/release/riku`
Run: `./target/release/riku --help`

**Step 4: Run all tests one final time**

Run: `cd /home/rakib/Code/others/piku/riku && cargo test`

**Step 5: Commit**

```bash
git add riku/
git commit -m "feat: release build configuration and final polish"
```

---

## Summary

| Phase | Tasks | Description |
|-------|-------|-------------|
| 1 | 1-3 | Scaffolding, paths, utilities |
| 2 | 4-6 | CLI framework with all commands |
| 3 | 7-8 | Deploy pipeline + all runtime handlers |
| 4 | 9-10 | Worker spawning + nginx config |
| 5 | 11-14 | Process supervisor daemon |
| 6 | 15-17 | Plugins, integration tests, polish |

**Total: 17 tasks across 6 phases.**

Each task has clear file paths, references to piku.py line numbers, and can be implemented independently within its phase. Phases should be done in order as later phases depend on earlier ones.
