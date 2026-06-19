//! Environment variable parsing, expansion, and configuration file utilities.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::display::echo;
use super::procfile::ENVVAR_RE;

/// Expand shell-style environment variables ($VAR and ${VAR}) in a buffer.
/// If var not found and no default, keep original text. If default given, use it.
pub fn expandvars(buffer: &str, env: &HashMap<String, String>, default: Option<&str>) -> String {
    ENVVAR_RE
        .replace_all(buffer, |caps: &regex::Captures| {
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
            // Strip null bytes and control characters (except common whitespace)
            let cleaned: String = v
                .chars()
                .filter(|c| !c.is_control() || *c == '\t')
                .collect();
            let expanded = expandvars(&cleaned, env, None);
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

/// Write key=value config file atomically (temp file + fsync + rename), so a
/// crash mid-write never leaves a truncated ENV file for the next reader.
pub fn write_config(filename: &Path, bag: &HashMap<String, String>, separator: &str) -> Result<()> {
    let mut content = String::new();
    for (k, v) in bag.iter() {
        content.push_str(&format!("{}{}{}\n", k, separator, v));
    }
    super::fs::write_atomic(filename, content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::NamedTempFile;

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
        writeln!(f).unwrap();
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

    #[test]
    fn test_parse_settings_strips_control_chars() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"KEY=hello\x00world\x01test").unwrap();
        f.flush().unwrap();
        let mut env = HashMap::new();
        let result = parse_settings(f.path(), &mut env).unwrap();
        assert_eq!(result.get("KEY").unwrap(), "helloworldtest");
    }

    #[test]
    fn test_write_config_default_separator() {
        let f = NamedTempFile::new().unwrap();
        let mut bag = HashMap::new();
        bag.insert("KEY1".into(), "val1".into());
        bag.insert("KEY2".into(), "val2".into());
        write_config(f.path(), &bag, "=").unwrap();
        let content = std::fs::read_to_string(f.path()).unwrap();
        assert!(content.contains("KEY1=val1\n"));
        assert!(content.contains("KEY2=val2\n"));
    }

    #[test]
    fn test_write_config_custom_separator() {
        let f = NamedTempFile::new().unwrap();
        let mut bag = HashMap::new();
        bag.insert("KEY".into(), "val".into());
        write_config(f.path(), &bag, ": ").unwrap();
        let content = std::fs::read_to_string(f.path()).unwrap();
        assert!(content.contains("KEY: val\n"));
    }
}
