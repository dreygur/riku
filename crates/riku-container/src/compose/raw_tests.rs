use super::*;
use crate::compose::default_compose_file;
use tempfile::TempDir;

fn app_with(compose: &str, dotenv: Option<&str>) -> TempDir {
    let app = TempDir::new().unwrap();
    std::fs::write(app.path().join("compose.yaml"), compose).unwrap();
    if let Some(dotenv) = dotenv {
        std::fs::write(app.path().join(".env"), dotenv).unwrap();
    }
    app
}

fn read(app: &TempDir, app_env: &[(&str, &str)]) -> Result<RawProject> {
    let compose = default_compose_file(app.path()).expect("compose.yaml was written");
    let app_env = app_env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    read_raw(app.path(), &compose, &app_env)
}

const INTERPOLATED: &str = "\
services:
  web:
    image: shop/web:${TAG}
    command: serve --port ${PORT:-5000} ${HOST:-0.0.0.0}
";

#[test]
fn default_and_dotenv_values_are_interpolated() {
    let app = app_with(INTERPOLATED, Some("TAG=1.4\n"));
    let project = read(&app, &[("PORT", "8000")]).unwrap();

    assert_eq!(project.services["web"]["image"], "shop/web:1.4");
    assert_eq!(
        project.services["web"]["command"],
        "serve --port 8000 0.0.0.0"
    );
    assert!(project.warnings.is_empty(), "{:?}", project.warnings);
}

#[test]
fn many_references_to_a_large_value_are_refused_at_the_document_cap() {
    // Each value stays under the cap; only the document total passes it.
    let big = format!("BIG={}\n", "x".repeat(1000 * 1000));
    let mut compose = String::from("services:\n  web:\n    image: shop/web\n    environment:\n");
    for i in 0..64 {
        compose.push_str(&format!("      - V{i}=${{BIG}}\n"));
    }
    let app = app_with(&compose, Some(&big));

    let err = match read(&app, &[]) {
        Ok(_) => panic!("64 MB of interpolated values accepted"),
        Err(e) => e.to_string(),
    };

    assert!(
        err.contains("compose.yaml") && err.contains("4194304 bytes"),
        "{err}"
    );
}

#[test]
fn app_env_overrides_dotenv() {
    let app = app_with(INTERPOLATED, Some("TAG=1.4\n"));
    let project = read(&app, &[("TAG", "1.5")]).unwrap();

    assert_eq!(project.services["web"]["image"], "shop/web:1.5");
}

#[test]
fn volumes_and_unset_variable_warning_are_reported_and_name_is_accepted() {
    let compose = "name: shop\nvolumes:\n  data:\nservices:\n  web:\n    image: ${NOPE}\n";
    let project = read(&app_with(compose, None), &[]).unwrap();

    assert!(project.volumes["data"].is_null());
    assert_eq!(
        project.warnings,
        ["variable NOPE is not set; using an empty string"]
    );
}

#[test]
fn unknown_service_key_names_key_and_service() {
    let compose = "services:\n  worker:\n    image: x\n    deploy:\n      replicas: 2\n";
    let err = read(&app_with(compose, None), &[]).unwrap_err().to_string();

    assert_eq!(err, "service 'worker': unsupported key 'deploy'");
}

const MERGED: &str = "\
x-base: &base
  restart: always
  image: nginx
services:
  web:
    <<: *base
    ports: [\"80:80\"]
";

#[test]
fn merge_keys_from_x_blocks_are_applied() {
    let project = read(&app_with(MERGED, None), &[]).unwrap();

    assert_eq!(project.services["web"]["restart"], "always");
    assert_eq!(project.services["web"]["image"], "nginx");
}

#[test]
fn unknown_key_arriving_only_through_a_merge_is_refused() {
    let compose = MERGED.replace("restart: always", "privileged: true");
    let err = read(&app_with(&compose, None), &[])
        .unwrap_err()
        .to_string();

    assert_eq!(err, "service 'web': unsupported key 'privileged'");
}

#[test]
fn custom_network_is_refused() {
    let compose = "networks:\n  backend: {}\nservices:\n  web:\n    image: x\n";
    let err = read(&app_with(compose, None), &[]).unwrap_err().to_string();

    assert!(err.contains("backend"), "{err}");
}

#[test]
fn compose_file_symlinked_outside_the_app_is_refused() {
    let app = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    std::fs::write(outside.path().join("other.yaml"), "services: {}\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("other.yaml"),
        app.path().join("compose.yaml"),
    )
    .unwrap();

    let err = read(&app, &[]).unwrap_err().to_string();
    assert!(err.contains("escapes"), "{err}");
}
