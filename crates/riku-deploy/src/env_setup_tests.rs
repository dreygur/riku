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

// --- setup_wsgi_env ---

#[test]
fn test_setup_wsgi_env_sets_nginx_wsgi() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "wsgiapp");

    let mut env = HashMap::new();
    setup_wsgi_env("wsgiapp", &paths, &mut env)?;

    assert_eq!(env.get("NGINX_WSGI").map(|s| s.as_str()), Some("true"));
    Ok(())
}

#[test]
fn test_setup_wsgi_env_sets_uwsgi_socket() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "wsgiapp");

    let mut env = HashMap::new();
    setup_wsgi_env("wsgiapp", &paths, &mut env)?;

    let socket = env.get("UWSGI_SOCKET").expect("UWSGI_SOCKET must be set");
    assert!(
        socket.contains("wsgiapp.sock"),
        "Socket path should contain app name"
    );
    Ok(())
}

#[test]
fn test_setup_wsgi_env_sets_socket_with_unix_prefix() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "wsgiapp");

    let mut env = HashMap::new();
    setup_wsgi_env("wsgiapp", &paths, &mut env)?;

    let socket = env.get("SOCKET").expect("SOCKET must be set");
    assert!(
        socket.starts_with("unix://"),
        "SOCKET should have unix:// prefix"
    );
    Ok(())
}

#[test]
fn test_setup_wsgi_env_persists_to_env_file() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "wsgiapp");

    let mut env = HashMap::new();
    setup_wsgi_env("wsgiapp", &paths, &mut env)?;

    let env_file = paths.env_root.join("wsgiapp").join("ENV");
    assert!(env_file.exists(), "ENV file should be created");
    let content = fs::read_to_string(&env_file)?;
    assert!(content.contains("NGINX_WSGI=true"));
    Ok(())
}

#[test]
fn test_setup_wsgi_env_idempotent() -> Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    setup_env_dir(&paths, "wsgiapp");

    let mut env = HashMap::new();
    // Call twice: should not duplicate the lines in the ENV file
    setup_wsgi_env("wsgiapp", &paths, &mut env)?;
    setup_wsgi_env("wsgiapp", &paths, &mut env)?;

    let env_file = paths.env_root.join("wsgiapp").join("ENV");
    let content = fs::read_to_string(&env_file)?;
    let count = content.matches("NGINX_WSGI=true").count();
    assert_eq!(count, 1, "NGINX_WSGI=true should appear exactly once");
    Ok(())
}
