use crate::engine::fixture::{logged, logging_engine};

const TOKEN: &str = "s3cret-token-value";

#[test]
fn login_token_never_in_argv() {
    let dir = tempfile::tempdir().unwrap();
    let stdin_copy = dir.path().join("stdin.txt");
    let (engine, log) = logging_engine(
        dir.path(),
        &format!(
            "case \"$1\" in login) cat > '{}';; esac",
            stdin_copy.display()
        ),
    );

    engine.login("registry.example", "deploy", TOKEN).unwrap();

    assert_eq!(
        logged(&log),
        ["login --username deploy --password-stdin registry.example"]
    );
    assert_eq!(std::fs::read_to_string(&stdin_copy).unwrap(), TOKEN);
}

#[test]
fn login_failure_reports_the_engine_reason_without_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(
        dir.path(),
        "cat >/dev/null; echo 'unauthorized' >&2; exit 1",
    );

    let err = engine
        .login("registry.example", "deploy", TOKEN)
        .unwrap_err()
        .to_string();

    assert!(err.contains("unauthorized"), "{err}");
    assert!(!err.contains(TOKEN), "token leaked into the error: {err}");
}

#[test]
fn login_refuses_values_that_could_be_read_as_flags() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "cat >/dev/null");

    assert!(engine.login("-x", "deploy", TOKEN).is_err());
    assert!(engine.login("registry.example", "", TOKEN).is_err());
    assert!(engine.login("registry.example", "deploy", "").is_err());
    assert!(
        logged(&log).is_empty(),
        "engine was run: {:?}",
        logged(&log)
    );
}
