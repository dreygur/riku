use super::*;
use crate::compose::test_support::{project_of, TestApp};
use std::path::Path;

const WITH_PGDATA: &str = "
services:
  db:
    image: postgres:16
    volumes: [\"pgdata:/var/lib/postgresql/data\"]
volumes:
  pgdata:
";

fn error_of(result: anyhow::Result<ComposeProject>) -> String {
    format!("{:#}", result.expect_err("project was accepted"))
}

#[test]
fn volume_declarations_that_escape_qualification_are_refused() {
    for (key, setting) in [
        ("name", "shop_media"),
        ("external", "true"),
        ("driver_opts", "{type: none, o: bind, device: /srv}"),
    ] {
        let text = format!("services: {{}}\nvolumes:\n  media:\n    {key}: {setting}\n");
        let message = error_of(project_of(&text));
        assert!(
            message.contains("media") && message.contains(key),
            "volume declaration with '{key}' should be refused naming both: {message}"
        );
    }
}

#[test]
fn volume_key_that_is_not_engine_safe_is_refused() {
    let message = error_of(project_of("services: {}\nvolumes:\n  \"-v\": {}\n"));
    assert!(message.contains("-v"), "{message}");
}

#[test]
fn volume_key_with_a_dot_is_refused() {
    let message = error_of(project_of(
        WITH_PGDATA.replace("pgdata", "pg.data").as_str(),
    ));
    assert!(
        message.contains("volume names may not contain '.'"),
        "{message}"
    );
}

#[test]
fn public_label_junk_is_refused_naming_service_and_value() {
    let text = "
services:
  web:
    image: nginx
    labels:
      riku.public: \"yes please\"
    ports: [\"8080:80\"]
";
    let message = error_of(project_of(text));
    assert!(
        message.contains("web") && message.contains("yes please"),
        "{message}"
    );
}

#[test]
fn debug_output_never_shows_environment_or_build_arg_values() {
    let app = TestApp::new("myapp");
    app.write(
        "compose.yaml",
        "
services:
  web:
    build:
      context: .
      args: {TOKEN: build-secret-value}
    environment: {DB_PASSWORD: env-secret-value}
",
    );

    let project = app.read(None, &[]).unwrap();
    let shown = format!("{project:?}");

    assert!(
        shown.contains("DB_PASSWORD"),
        "names should be listed: {shown}"
    );
    assert!(
        !shown.contains("env-secret-value") && !shown.contains("build-secret-value"),
        "Debug leaked a secret value: {shown}"
    );
}

#[test]
fn dockerfile_app_is_one_service_named_app() {
    let project = single_image_project(Path::new("/srv/myapp"), Path::new("Dockerfile")).unwrap();

    assert_eq!(project.services.len(), 1);
    let service = &project.services[0];
    assert_eq!(service.name.as_str(), "app");
    let build = service.build.as_ref().expect("app service has a build");
    assert_eq!(build.context, Path::new("/srv/myapp"));
    assert_eq!(build.dockerfile.as_deref(), Some(Path::new("Dockerfile")));
    assert!(service.ports.is_empty());
}
