use anyhow::Result;
use std::collections::HashMap;

use crate::config::RikuPaths;
use crate::util::{echo, exit_if_invalid, parse_settings};

/// Scale workers — parse SCALING file, compute deltas, deploy.
pub fn cmd_ps_scale(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("SCALING");
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
                            &format!("Adding new worker type '{}' with count {}", k, c),
                            "green",
                        );
                        deltas.insert(k, c);
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

    crate::deploy::do_deploy(&app, paths, &deltas, None)?;
    Ok(())
}
