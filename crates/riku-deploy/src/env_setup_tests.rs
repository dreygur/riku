use super::*;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    crate::config::RikuPaths::for_tests(tmp.path())
}

fn setup_env_dir(paths: &RikuPaths, app: &str) {
    std::fs::create_dir_all(paths.env_root.join(app)).unwrap();
    std::fs::create_dir_all(&paths.nginx_root).unwrap();
}

// --- write_live_env ---

#[test]
fn test_write_live_env_creates_file() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "myapp");

    let env = HashMap::new();
    write_live_env("myapp", &paths, &env)?;

    let live_env_path = paths.env_root.join("myapp").join("LIVE_ENV");
    assert!(live_env_path.exists());
    Ok(())
}

#[test]
fn live_env_is_written_for_an_app_that_never_had_config_set() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    assert!(
        !paths.env_root.join("myapp").exists(),
        "fixture must start without an env dir"
    );

    write_live_env("myapp", &paths, &HashMap::new())?;

    assert!(paths.env_root.join("myapp").join("LIVE_ENV").exists());
    Ok(())
}

#[test]
fn test_write_live_env_contains_app_name() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "myapp");

    let env = HashMap::new();
    write_live_env("myapp", &paths, &env)?;

    let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
    assert!(content.contains("APP=myapp"));
    Ok(())
}

#[test]
fn test_write_live_env_includes_in_memory_vars() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "myapp");

    let mut env = HashMap::new();
    env.insert(
        "DATABASE_URL".to_string(),
        "postgres://localhost/db".to_string(),
    );
    write_live_env("myapp", &paths, &env)?;

    let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
    assert!(content.contains("DATABASE_URL=postgres://localhost/db"));
    Ok(())
}

#[test]
fn test_write_live_env_reads_existing_env_file() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "myapp");

    // Write an ENV file ahead of time
    let env_file = paths.env_root.join("myapp").join("ENV");
    fs::write(&env_file, "SECRET_KEY=abc123\n")?;

    let env = HashMap::new();
    write_live_env("myapp", &paths, &env)?;

    let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
    assert!(content.contains("SECRET_KEY=abc123"));
    Ok(())
}

#[test]
fn live_env_and_its_dir_are_owner_only() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env_dir = paths.env_root.join("myapp");
    setup_env_dir(&paths, "myapp");
    fs::set_permissions(&env_dir, fs::Permissions::from_mode(0o755))?;

    write_live_env("myapp", &paths, &HashMap::new())?;

    let mode = |path: &std::path::Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let live_env = env_dir.join("LIVE_ENV");
    assert_eq!(
        mode(&live_env),
        0o600,
        "LIVE_ENV is mode {:o}",
        mode(&live_env)
    );
    assert_eq!(
        mode(&env_dir),
        0o700,
        "env dir is mode {:o}",
        mode(&env_dir)
    );
    Ok(())
}

#[test]
fn test_write_live_env_skips_comment_lines_in_env_file() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "myapp");

    let env_file = paths.env_root.join("myapp").join("ENV");
    fs::write(&env_file, "# This is a comment\nFOO=bar\n")?;

    let env = HashMap::new();
    write_live_env("myapp", &paths, &env)?;

    let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
    assert!(!content.contains("# This is a comment"));
    assert!(content.contains("FOO=bar"));
    Ok(())
}

#[test]
fn test_write_live_env_contains_log_root() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "myapp");

    let env = HashMap::new();
    write_live_env("myapp", &paths, &env)?;

    let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
    assert!(content.contains("LOG_ROOT="));
    Ok(())
}
