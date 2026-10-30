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
fn derived_name_qualifies_volumes_directly() {
    let project = project_of(WITH_PGDATA).unwrap();

    assert_eq!(project.name_source, ProjectNameSource::Derived);
    assert_eq!(
        existing_volume_name(&project, "pgdata").unwrap().as_deref(),
        Some("shop_pgdata")
    );
    assert_eq!(existing_volume_name(&project, "undeclared").unwrap(), None);
}

#[test]
fn project_name_follows_compose_file_directory_but_needs_confirmation() {
    let app = TestApp::new("myapp");
    app.write("deploy/compose.yml", WITH_PGDATA);

    let mut project = app.read(Some("deploy/compose.yml"), &[]).unwrap();

    assert_eq!(
        project.name_source,
        ProjectNameSource::Declared,
        "another app with deploy/compose.yml would get the same project name"
    );
    project.confirm_declared_name();
    assert_eq!(
        existing_volume_name(&project, "pgdata").unwrap().as_deref(),
        Some("deploy_pgdata"),
        "an app with its compose file in deploy/ was run by compose as project 'deploy'"
    );
}

#[test]
fn declared_name_is_recorded_and_needs_confirmation_before_qualifying_volumes() {
    let app = TestApp::new("myapp");
    app.write("compose.yaml", &format!("name: shop\n{WITH_PGDATA}"));

    let mut project = app.read(None, &[]).unwrap();

    assert_eq!(project.project_name, "shop");
    assert_eq!(project.name_source, ProjectNameSource::Declared);
    let refusal = format!(
        "{:#}",
        existing_volume_name(&project, "pgdata").unwrap_err()
    );
    assert!(
        refusal.contains("shop"),
        "refusal should name the project: {refusal}"
    );

    project.confirm_declared_name();

    assert_eq!(
        existing_volume_name(&project, "pgdata").unwrap().as_deref(),
        Some("shop_pgdata")
    );
}

#[test]
fn compose_project_name_env_wins_and_is_normalised() {
    let app = TestApp::new("myapp");
    app.write("compose.yaml", &format!("name: other\n{WITH_PGDATA}"));

    let mut project = app
        .read(None, &[("COMPOSE_PROJECT_NAME", "Shop.Prod")])
        .unwrap();
    project.confirm_declared_name();

    assert_eq!(project.name_source, ProjectNameSource::DeclaredConfirmed);
    assert_eq!(
        existing_volume_name(&project, "pgdata").unwrap().as_deref(),
        Some("shopprod_pgdata")
    );
}

#[test]
fn junk_project_names_are_refused_naming_the_value() {
    let app = TestApp::new("myapp");
    app.write("compose.yaml", "name: \"../shop\"\nservices: {}\n");
    let declared = error_of(app.read(None, &[]));
    assert!(declared.contains("../shop"), "{declared}");

    app.write("compose.yaml", "services: {}\n");
    let from_env = error_of(app.read(None, &[("COMPOSE_PROJECT_NAME", "...")]));
    assert!(from_env.contains("..."), "{from_env}");
}

#[test]
fn compose_project_name_in_dotenv_is_declared() {
    let app = TestApp::new("myapp");
    app.write("compose.yaml", WITH_PGDATA);
    app.write(".env", "COMPOSE_PROJECT_NAME=Shop.Prod\n");

    let mut project = app.read(None, &[]).unwrap();

    assert_eq!(project.project_name, "shopprod");
    assert_eq!(project.name_source, ProjectNameSource::Declared);
    project.confirm_declared_name();
    assert_eq!(
        existing_volume_name(&project, "pgdata").unwrap().as_deref(),
        Some("shopprod_pgdata")
    );
}

#[test]
fn app_env_overrides_dotenv_project_name() {
    let app = TestApp::new("myapp");
    app.write("compose.yaml", WITH_PGDATA);
    app.write(".env", "COMPOSE_PROJECT_NAME=fromdotenv\n");

    let project = app
        .read(None, &[("COMPOSE_PROJECT_NAME", "fromenv")])
        .unwrap();

    assert_eq!(project.project_name, "fromenv");
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
    let project =
        single_image_project("myapp", Path::new("/srv/myapp"), Path::new("Dockerfile")).unwrap();

    assert_eq!(project.services.len(), 1);
    let service = &project.services[0];
    assert_eq!(service.name.as_str(), "app");
    let build = service.build.as_ref().expect("app service has a build");
    assert_eq!(build.context, Path::new("/srv/myapp"));
    assert_eq!(build.dockerfile.as_deref(), Some(Path::new("Dockerfile")));
    assert!(service.ports.is_empty());
}

#[test]
fn dockerfile_apps_get_project_names_of_their_own() {
    let dockerfile = Path::new("Dockerfile");
    let first = single_image_project("myapp", Path::new("/srv/myapp"), dockerfile).unwrap();
    let second = single_image_project("shop", Path::new("/srv/shop"), dockerfile).unwrap();

    assert_eq!(first.project_name(), "myapp");
    assert_ne!(first.project_name(), second.project_name());
    assert!(single_image_project("-bad", Path::new("/srv/x"), dockerfile).is_err());
}
