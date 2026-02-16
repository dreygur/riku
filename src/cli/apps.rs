use anyhow::Result;
use colored::Colorize;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::config::{RikuPaths, RIKU_RAW_SOURCE_URL};
use crate::supervisor::Supervisor;
use crate::util::{echo, exit_if_invalid, parse_settings, write_config};

/// List apps, marking running ones with '*'.
pub fn cmd_apps(paths: &RikuPaths) -> Result<()> {
    let app_root = &paths.app_root;
    if !app_root.exists() {
        echo("There are no applications deployed.", "");
        return Ok(());
    }

    let mut apps: Vec<String> = fs::read_dir(app_root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if apps.is_empty() {
        echo("There are no applications deployed.", "");
        return Ok(());
    }

    apps.sort();

    for a in &apps {
        // Check for running worker configs (*.ini or *.toml) in workers_enabled
        let ini_pattern = paths.workers_enabled.join(format!("{}*.ini", a));
        let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", a));
        let ini_matches = glob::glob(ini_pattern.to_str().unwrap_or(""))
            .map(|g| g.count())
            .unwrap_or(0);
        let toml_matches = glob::glob(toml_pattern.to_str().unwrap_or(""))
            .map(|g| g.count())
            .unwrap_or(0);
        let running = ini_matches + toml_matches > 0;
        let prefix = if running { "*" } else { " " };
        println!("{}", format!("{}{}", prefix, a).green());
    }

    Ok(())
}

/// Show app configuration (ENV file).
pub fn cmd_config_show(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
        println!("{}", content.trim().white());
    } else {
        echo(
            &format!("Warning: app '{}' not deployed, no config found.", app),
            "yellow",
        );
    }
    Ok(())
}

/// Get a single config value.
pub fn cmd_config_get(paths: &RikuPaths, app: &str, key: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let mut env = HashMap::new();
        let settings = parse_settings(&config_file, &mut env)?;
        if let Some(val) = settings.get(key) {
            println!("{}", val.white());
        }
    } else {
        echo(
            &format!("Warning: no active configuration for '{}'", app),
            "",
        );
    }
    Ok(())
}

/// Set config values (KEY=VALUE pairs), write config, trigger deploy.
pub fn cmd_config_set(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    // Join all settings and split them shell-style
    let joined = settings.join(" ");
    let parts = shell_split(&joined);

    for s in &parts {
        if let Some(eq_pos) = s.find('=') {
            let k = s[..eq_pos].trim().to_string();
            let v = s[eq_pos + 1..].trim().to_string();
            println!("{}", format!("Setting {}={} for '{}'", k, v, app).white());
            env.insert(k, v);
        } else {
            echo(&format!("Error: malformed setting '{}'", s), "red");
            return Ok(());
        }
    }
    write_config(&config_file, &env, "=")?;
    // Trigger a deploy after config change
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    crate::deploy::do_deploy(&app, paths, &deltas, None)?;
    Ok(())
}

/// Unset config values, write config, trigger deploy.
pub fn cmd_config_unset(paths: &RikuPaths, app: &str, keys: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    for s in keys {
        if env.remove(s).is_some() {
            println!("{}", format!("Unsetting {} for '{}'", s, app).white());
        }
    }
    write_config(&config_file, &env, "=")?;
    // Trigger a deploy after config change
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    crate::deploy::do_deploy(&app, paths, &deltas, None)?;
    Ok(())
}

/// Show live running configuration (LIVE_ENV file).
pub fn cmd_config_live(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let live_config = paths.env_root.join(&app).join("LIVE_ENV");
    if live_config.exists() {
        let content = fs::read_to_string(&live_config)?;
        println!("{}", content.trim().white());
    } else {
        echo(
            &format!("Warning: app '{}' not deployed, no config found.", app),
            "yellow",
        );
    }
    Ok(())
}

/// Deploy an app.
pub fn cmd_deploy(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    crate::deploy::do_deploy(&app, paths, &deltas, None)
}

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
            let pattern = dir.join(format!("{}*.{}", app, ext));
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

    // Remove nginx files
    for ext in &["conf", "sock", "key", "crt"] {
        let f = paths.nginx_root.join(format!("{}.{}", app, ext));
        if f.exists() {
            echo(&format!("--> Removing file '{}'", f.display()), "yellow");
            fs::remove_file(&f)?;
        }
    }

    // Remove ACME certs if they exist
    let acme_link = paths.acme_www.join(&app);
    if acme_link.exists() {
        let acme_certs = fs::canonicalize(&acme_link).unwrap_or_else(|_| acme_link.clone());
        if acme_certs.exists() {
            echo(
                &format!("--> Removing folder '{}'", acme_certs.display()),
                "yellow",
            );
            fs::remove_dir_all(&acme_certs)?;
        }
        echo(
            &format!("--> Removing file '{}'", acme_link.display()),
            "yellow",
        );
        // acme_link is a symlink, so remove_file works
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

/// Tail app log files using multi_tail.
pub fn cmd_logs(paths: &RikuPaths, app: &str, process: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let pattern = paths.log_root.join(&app).join(format!("{}.*.log", process));
    let logfiles: Vec<String> = glob::glob(pattern.to_str().unwrap_or(""))
        .map(|g| {
            g.filter_map(|e| e.ok().map(|p| p.to_string_lossy().to_string()))
                .collect()
        })
        .unwrap_or_default();

    if !logfiles.is_empty() {
        multi_tail(&logfiles)?;
    } else {
        echo(&format!("No logs found for app '{}'.", app), "yellow");
    }
    Ok(())
}

/// Show process scaling info.
pub fn cmd_ps_show(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("SCALING");
    if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
        println!("{}", content.trim().white());
    } else {
        echo(
            &format!("Error: no workers found for app '{}'.", app),
            "red",
        );
    }
    Ok(())
}

/// Scale workers — parse SCALING file, compute deltas, deploy.
pub fn cmd_ps_scale(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("SCALING");
    // Parse the scaling file as a procfile-style k:v format
    let mut env = HashMap::new();
    let worker_count = parse_settings(&config_file, &mut env)?;

    let mut deltas: HashMap<String, i64> = HashMap::new();
    for s in settings {
        if let Some(eq_pos) = s.find('=') {
            let k = s[..eq_pos].trim().to_string();
            let v_str = s[eq_pos + 1..].trim().to_string();
            match v_str.parse::<i64>() {
                Ok(c) => {
                    if c < 0 {
                        echo(&format!("Error: cannot scale type '{}' below 0", k), "red");
                        return Ok(());
                    }
                    if let Some(current) = worker_count.get(&k) {
                        match current.parse::<i64>() {
                            Ok(current_val) => {
                                deltas.insert(k, c - current_val);
                            }
                            Err(_) => {
                                echo(&format!("Error: malformed setting '{}'", s), "red");
                                return Ok(());
                            }
                        }
                    } else {
                        echo(
                            &format!("Error: worker type '{}' not present in '{}'", k, app),
                            "red",
                        );
                        return Ok(());
                    }
                }
                Err(_) => {
                    echo(&format!("Error: malformed setting '{}'", s), "red");
                    return Ok(());
                }
            }
        } else {
            echo(&format!("Error: malformed setting '{}'", s), "red");
            return Ok(());
        }
    }

    // Call do_deploy with the calculated deltas
    crate::deploy::do_deploy(&app, paths, &deltas, None)?;
    Ok(())
}

/// Run a command in the app context with LIVE_ENV loaded.
pub fn cmd_run(paths: &RikuPaths, app: &str, cmd: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("LIVE_ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    let app_dir = paths.app_root.join(&app);
    let shell_cmd = cmd.join(" ");

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&shell_cmd)
        .current_dir(&app_dir)
        .envs(&env)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    child.wait()?;
    Ok(())
}

/// Restart an app: stop then spawn.
pub fn cmd_restart(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    echo(&format!("restarting app '{}'...", app), "yellow");
    do_stop(paths, &app);
    // Trigger a deploy to restart the app
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    crate::deploy::do_deploy(&app, paths, &deltas, None)
}

/// Stop an app by removing enabled worker configs.
pub fn cmd_stop(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;
    do_stop(paths, &app);
    Ok(())
}

/// Self-update the binary by downloading latest from RIKU_RAW_SOURCE_URL.
pub fn cmd_update() -> Result<()> {
    echo("Updating riku...", "");

    let output = Command::new("curl")
        .args([
            "-sL",
            "-w",
            "%{http_code}",
            RIKU_RAW_SOURCE_URL,
            "-o",
            "/dev/null",
        ])
        .output()?;

    let http_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if http_code == "200" {
        echo(
            "Note: self-update for riku binary is not yet implemented.",
            "yellow",
        );
        echo("The piku.py reference source is accessible.", "");
    } else {
        echo(
            &format!(
                "Error updating riku - please check if {} is accessible from this machine.",
                RIKU_RAW_SOURCE_URL
            ),
            "",
        );
    }
    echo("Done.", "");
    Ok(())
}

/// Start the supervisor daemon.
pub fn cmd_supervisor(paths: &RikuPaths) -> Result<()> {
    let mut supervisor = Supervisor::new(paths.workers_enabled.clone())?;
    supervisor.run()
}

// --- Internal helpers ---

/// Stop an app by removing its enabled worker config files.
fn do_stop(paths: &RikuPaths, app: &str) {
    let mut configs: Vec<std::path::PathBuf> = Vec::new();

    for ext in &["ini", "toml"] {
        let pattern = paths.workers_enabled.join(format!("{}*.{}", app, ext));
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            for entry in entries.flatten() {
                configs.push(entry);
            }
        }
    }

    if !configs.is_empty() {
        echo(&format!("Stopping app '{}'...", app), "yellow");
        for c in &configs {
            let _ = fs::remove_file(c);
        }
    } else {
        echo(&format!("Error: app '{}' not deployed!", app), "red");
    }
}

/// Simple shell-like splitting of a string on whitespace, respecting quotes.
fn shell_split(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;

    for c in input.chars() {
        if escape_next {
            current.push(c);
            escape_next = false;
            continue;
        }
        if c == '\\' && !in_single_quote {
            escape_next = true;
            continue;
        }
        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            continue;
        }
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            continue;
        }
        if c.is_whitespace() && !in_single_quote && !in_double_quote {
            if !current.is_empty() {
                result.push(current.clone());
                current.clear();
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// Tail multiple log files, showing the last `catch_up` lines then polling.
fn multi_tail(filenames: &[String]) -> Result<()> {
    let catch_up: usize = 20;

    // Compute prefixes (filename stem without extension)
    let prefixes: Vec<String> = filenames
        .iter()
        .map(|f| {
            Path::new(f)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let longest = prefixes.iter().map(|p| p.len()).max().unwrap_or(0);

    // Catch up: show last `catch_up` lines from each file
    for (i, f) in filenames.iter().enumerate() {
        if let Ok(file) = fs::File::open(f) {
            let reader = BufReader::new(file);
            let lines: VecDeque<String> = reader
                .lines()
                .filter_map(|l| l.ok())
                .collect::<VecDeque<String>>();
            // Take last catch_up lines
            let start = if lines.len() > catch_up {
                lines.len() - catch_up
            } else {
                0
            };
            for line in lines.iter().skip(start) {
                println!(
                    "{}",
                    format!(
                        "{} | {}",
                        prefixes[i].as_str().to_string()
                            + &" ".repeat(longest.saturating_sub(prefixes[i].len())),
                        line
                    )
                    .white()
                );
            }
        }
    }

    // Open files at the end for tailing
    let mut files: Vec<fs::File> = Vec::new();
    let mut inodes: Vec<u64> = Vec::new();
    for f in filenames {
        let mut file = fs::File::open(f)?;
        let meta = file.metadata()?;
        inodes.push(meta.ino());
        file.seek(SeekFrom::End(0))?;
        files.push(file);
    }

    let mut active_filenames: Vec<String> = filenames.to_vec();

    loop {
        let mut updated = false;

        for i in 0..active_filenames.len() {
            let mut buf = String::new();
            if files[i].read_to_string(&mut buf).is_ok() && !buf.is_empty() {
                updated = true;
                for line in buf.lines() {
                    println!(
                        "{}",
                        format!("{:<width$} | {}", prefixes[i], line, width = longest).white()
                    );
                }
            }
        }

        if !updated {
            thread::sleep(Duration::from_secs(1));
            // Check for log rotation
            let mut i = 0;
            while i < active_filenames.len() {
                let f = &active_filenames[i];
                if Path::new(f).exists() {
                    if let Ok(meta) = fs::metadata(f) {
                        if meta.ino() != inodes[i] {
                            // Log rotated, reopen
                            if let Ok(mut new_file) = fs::File::open(f) {
                                let _ = new_file.seek(SeekFrom::Start(0));
                                files[i] = new_file;
                                inodes[i] = meta.ino();
                            }
                        }
                    }
                    i += 1;
                } else {
                    active_filenames.remove(i);
                    files.remove(i);
                    inodes.remove(i);
                    // Don't increment i since we removed an element
                }
            }
            if active_filenames.is_empty() {
                break;
            }
        }
    }

    Ok(())
}
