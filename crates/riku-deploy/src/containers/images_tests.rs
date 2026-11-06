use std::collections::BTreeMap;
use std::fs;

use super::*;
use crate::containers::AppName;
use crate::mock_engine::{logged, mock_engine, project_in};

const TWO_SERVICES: &str = "
services:
  db:
    image: postgres:16
  web:
    build: .
";

fn env_of(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn build_tag_separates_apps_that_would_join_to_the_same_text() {
    let tag = |app: &str, service: &str| {
        build_tag(
            &AppName::try_from(app).unwrap(),
            &ServiceName::try_from(service).unwrap(),
        )
    };

    assert_ne!(tag("a-b", "c"), tag("a", "b-c"));
    assert_ne!(tag("Shop", "web"), tag("shop", "web"));
    let repo = tag("myapp", "web");
    let hash = repo
        .strip_prefix("localhost/riku-")
        .expect("tag lacks localhost/riku- prefix");
    assert!(
        hash.len() == 16 && hash.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
        "not a lowercase hex repository: {repo}"
    );
}

#[test]
fn pulls_image_services_and_builds_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let (app_path, project) = project_in(dir.path(), "myapp", TWO_SERVICES);
    fs::write(app_path.join("Dockerfile"), "FROM scratch\n").unwrap();
    let (engine, log) = mock_engine(dir.path(), "");
    let app = AppName::try_from("myapp").unwrap();

    prepare_images(&engine, &app, &project, &BTreeMap::new()).unwrap();

    let lines = logged(&log);
    assert!(lines.contains(&"pull postgres:16".to_string()), "{lines:?}");
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("build") && l.contains(" -t localhost/riku-")),
        "{lines:?}"
    );
}

#[test]
fn token_only_reaches_login_on_stdin() {
    let dir = tempfile::tempdir().unwrap();
    let (_, project) = project_in(
        dir.path(),
        "myapp",
        "services:\n  db:\n    image: ghcr.io/o/db:1\n",
    );
    let stdin_copy = dir.path().join("stdin.txt");
    let behaviour = format!(
        "case \"$1\" in login) cat > '{}';; esac",
        stdin_copy.display()
    );
    let (engine, log) = mock_engine(dir.path(), &behaviour);
    let app = AppName::try_from("myapp").unwrap();
    let env = env_of(&[
        ("RIKU_GHCR_USERNAME", "robot"),
        ("RIKU_GHCR_TOKEN", "s3cret"),
    ]);

    prepare_images(&engine, &app, &project, &env).unwrap();

    let lines = logged(&log);
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("login --username robot")),
        "{lines:?}"
    );
    assert!(
        lines.iter().all(|l| !l.contains("s3cret")),
        "token appeared in argv: {lines:?}"
    );
    assert_eq!(fs::read_to_string(stdin_copy).unwrap(), "s3cret");
}

#[test]
fn half_configured_registry_login_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (_, project) = project_in(
        dir.path(),
        "myapp",
        "services:\n  db:\n    image: postgres:16\n",
    );
    let (engine, _log) = mock_engine(dir.path(), "");
    let app = AppName::try_from("myapp").unwrap();
    let env = env_of(&[("RIKU_GHCR_TOKEN", "s3cret")]);

    let err = prepare_images(&engine, &app, &project, &env).unwrap_err();

    assert!(format!("{err:#}").contains("RIKU_GHCR_USERNAME"), "{err:#}");
}
