//! Riku runtime plugin: Container
//!
//! Handles apps with a `Dockerfile`, `Containerfile`, or a compose file
//! (`compose.yaml`, `compose.yml`, `docker-compose.yaml`, `docker-compose.yml`).
//! Auto-detects Docker or Podman. Subcommands: detect, build, env, start,
//! pull-service.
//!
//! Compose apps pull pre-built images rather than building from a local
//! Dockerfile. If `GHCR_USERNAME`/`GHCR_TOKEN` are set in the app's ENV file,
//! the plugin logs in to `ghcr.io` before pulling.

use anyhow::{anyhow, bail, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const COMPOSE_FILE_NAMES: [&str; 4] = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let arg2 = std::env::args().nth(2);
    let app_path = std::env::var("RIKU_APP_PATH").unwrap_or_else(|_| ".".into());
    let app = std::env::var("RIKU_APP").unwrap_or_else(|_| "app".into());
    let env_path = std::env::var("RIKU_ENV_PATH").ok().map(PathBuf::from);
    let app_path = Path::new(&app_path);

    match cmd.as_str() {
        "detect" => detect(app_path),
        "build" => build(app_path, &app, env_path.as_deref()),
        "env" => print_env(app_path, &app),
        "start" => print_start(app_path, &app),
        "pull-service" => {
            let service = arg2.ok_or_else(|| anyhow!("pull-service requires a service name"))?;
            pull_service(app_path, &service, env_path.as_deref())
        }
        other => bail!("Unknown subcommand: {}", other),
    }
}

/// Find the app's compose file, following docker compose's own precedence.
fn compose_file(app_path: &Path) -> Option<PathBuf> {
    COMPOSE_FILE_NAMES
        .iter()
        .map(|name| app_path.join(name))
        .find(|path| path.exists())
}

fn detect(app_path: &Path) -> Result<()> {
    if app_path.join("Dockerfile").exists()
        || app_path.join("Containerfile").exists()
        || compose_file(app_path).is_some()
    {
        std::process::exit(0);
    }
    std::process::exit(1);
}

fn runtime() -> &'static str {
    if which("podman") {
        "podman"
    } else {
        "docker"
    }
}

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Program and leading args for invoking compose with the active container runtime.
fn compose_cmd() -> (&'static str, Vec<&'static str>) {
    if runtime() == "podman" {
        if which("podman-compose") {
            return ("podman-compose", vec![]);
        }
        return ("podman", vec!["compose"]);
    }
    ("docker", vec!["compose"])
}

fn compose_cmd_display((program, base_args): &(&'static str, Vec<&'static str>)) -> String {
    if base_args.is_empty() {
        program.to_string()
    } else {
        format!("{} {}", program, base_args.join(" "))
    }
}

/// Read `KEY=VALUE` lines from the app's ENV file (ignoring comments/blanks).
/// Returns an empty map if there is no env path or no file yet.
fn read_env_file(env_path: Option<&Path>) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    let Some(env_path) = env_path else {
        return vars;
    };
    let Ok(content) = fs::read_to_string(env_path.join("ENV")) else {
        return vars;
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            vars.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    vars
}

/// Log in to GHCR if `GHCR_USERNAME`/`GHCR_TOKEN` are configured. No-op otherwise.
fn ghcr_login(env_path: Option<&Path>) -> Result<()> {
    let vars = read_env_file(env_path);
    let (Some(user), Some(token)) = (vars.get("GHCR_USERNAME"), vars.get("GHCR_TOKEN")) else {
        return Ok(());
    };

    let mut child = Command::new(runtime())
        .args(["login", "ghcr.io", "-u", user, "--password-stdin"])
        .stdin(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("child spawned with piped stdin")
        .write_all(token.as_bytes())?;
    if !child.wait()?.success() {
        bail!("GHCR login failed");
    }
    Ok(())
}

fn build(app_path: &Path, app: &str, env_path: Option<&Path>) -> Result<()> {
    if let Some(compose) = compose_file(app_path) {
        ghcr_login(env_path)?;
        let (program, base_args) = compose_cmd();
        let status = Command::new(program)
            .args(&base_args)
            .args(["-f", &compose.file_name().unwrap().to_string_lossy(), "pull"])
            .current_dir(app_path)
            .status()?;
        if !status.success() {
            bail!("compose pull failed");
        }
        return Ok(());
    }

    let rt = runtime();
    let image = format!("riku-{}", app);

    let dockerfile = if app_path.join("Containerfile").exists() {
        "Containerfile"
    } else {
        "Dockerfile"
    };

    let status = Command::new(rt)
        .args(["build", "-t", &image, "-f", dockerfile, "."])
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        bail!("Container build failed");
    }
    Ok(())
}

fn print_env(app_path: &Path, app: &str) -> Result<()> {
    if let Some(compose) = compose_file(app_path) {
        println!("CONTAINER_RUNTIME={}", runtime());
        println!(
            "COMPOSE_FILE={}",
            compose.file_name().unwrap().to_string_lossy()
        );
        return Ok(());
    }
    let image = format!("riku-{}", app);
    println!("CONTAINER_IMAGE={}", image);
    println!("CONTAINER_RUNTIME={}", runtime());
    Ok(())
}

fn print_start(app_path: &Path, app: &str) -> Result<()> {
    if let Some(compose) = compose_file(app_path) {
        let cmd = compose_cmd();
        println!(
            "{} -f {} up",
            compose_cmd_display(&cmd),
            compose.file_name().unwrap().to_string_lossy()
        );
        return Ok(());
    }

    let rt = runtime();
    let image = format!("riku-{}", app);
    println!("{} run --rm -p $PORT:$PORT {}", rt, image);
    Ok(())
}

/// Pull and recreate a single compose service, without touching the rest of
/// the stack. Used by the GHCR webhook to apply a freshly pushed image.
fn pull_service(app_path: &Path, service: &str, env_path: Option<&Path>) -> Result<()> {
    let Some(compose) = compose_file(app_path) else {
        bail!("No compose file found in {}", app_path.display());
    };
    ghcr_login(env_path)?;

    let (program, base_args) = compose_cmd();
    let compose_filename = compose.file_name().unwrap().to_string_lossy().into_owned();

    let status = Command::new(program)
        .args(&base_args)
        .args(["-f", &compose_filename, "pull", service])
        .current_dir(app_path)
        .status()?;
    if !status.success() {
        bail!("compose pull failed for service '{}'", service);
    }

    let status = Command::new(program)
        .args(&base_args)
        .args(["-f", &compose_filename, "up", "-d", "--no-deps", service])
        .current_dir(app_path)
        .status()?;
    if !status.success() {
        bail!("compose up failed for service '{}'", service);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn temp_app_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "riku-plugin-container-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn compose_file_prefers_compose_yaml_over_docker_compose_yml() {
        let dir = temp_app_dir();
        File::create(dir.join("docker-compose.yml")).unwrap();
        File::create(dir.join("compose.yaml")).unwrap();

        assert_eq!(compose_file(&dir), Some(dir.join("compose.yaml")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn compose_file_finds_docker_compose_yml_alone() {
        let dir = temp_app_dir();
        File::create(dir.join("docker-compose.yml")).unwrap();

        assert_eq!(compose_file(&dir), Some(dir.join("docker-compose.yml")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn compose_file_none_when_absent() {
        let dir = temp_app_dir();
        assert_eq!(compose_file(&dir), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detect_exits_0_for_compose_app() {
        let dir = temp_app_dir();
        File::create(dir.join("compose.yml")).unwrap();
        // detect() calls process::exit, so we only check compose_file directly
        // here rather than forking a process in a unit test.
        assert!(compose_file(&dir).is_some());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn compose_cmd_display_formats_with_base_args() {
        assert_eq!(
            compose_cmd_display(&("docker", vec!["compose"])),
            "docker compose"
        );
        assert_eq!(
            compose_cmd_display(&("podman-compose", vec![])),
            "podman-compose"
        );
    }

    #[test]
    fn read_env_file_parses_ghcr_credentials() {
        let dir = temp_app_dir();
        fs::write(dir.join("ENV"), "GHCR_USERNAME=octocat\nGHCR_TOKEN=ghp_abc123\n# comment\n\n")
            .unwrap();

        let vars = read_env_file(Some(&dir));
        assert_eq!(vars.get("GHCR_USERNAME").unwrap(), "octocat");
        assert_eq!(vars.get("GHCR_TOKEN").unwrap(), "ghp_abc123");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_env_file_empty_when_no_env_path() {
        assert!(read_env_file(None).is_empty());
    }
}
