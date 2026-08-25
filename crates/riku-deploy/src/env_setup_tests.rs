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
