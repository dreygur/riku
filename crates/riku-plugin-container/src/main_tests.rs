use super::*;
use std::fs::File;
use tempfile::TempDir;

#[test]
fn compose_file_env_claims_the_app_even_when_invalid() {
    let tmp = TempDir::new().unwrap();
    File::create(tmp.path().join("package.json")).unwrap();

    assert!(!handles_app(tmp.path(), None));
    assert!(handles_app(tmp.path(), Some("missing.yml")));
}

#[test]
fn read_env_file_parses_ghcr_credentials() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(
        dir.join("ENV"),
        "RIKU_GHCR_USERNAME=octocat\nRIKU_GHCR_TOKEN=ghp_abc123\n# comment\n\n",
    )
    .unwrap();

    let vars = read_env_file(Some(dir));
    assert_eq!(vars.get("RIKU_GHCR_USERNAME").unwrap(), "octocat");
    assert_eq!(vars.get("RIKU_GHCR_TOKEN").unwrap(), "ghp_abc123");
}

#[test]
fn read_env_file_empty_when_no_env_path() {
    assert!(read_env_file(None).is_empty());
}
