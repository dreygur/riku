use crate::compose::test_support::TestApp;
use crate::compose::{single_image_project, ProjectNameSource};
use std::path::Path;

fn compose_source(app_name: &str) -> (String, ProjectNameSource) {
    let app = TestApp::new(app_name);
    app.write("compose.yaml", "services: {}\n");
    let project = app.read(None, &[]).unwrap();
    (project.project_name().to_string(), project.name_source())
}

fn single_image_source(app_name: &str) -> (String, ProjectNameSource) {
    let project =
        single_image_project(app_name, Path::new("/srv/app"), Path::new("Dockerfile")).unwrap();
    (project.project_name().to_string(), project.name_source())
}

#[test]
fn app_name_that_is_already_a_project_name_is_derived() {
    for source in [compose_source, single_image_source] {
        assert_eq!(
            source("shop"),
            ("shop".to_string(), ProjectNameSource::Derived)
        );
    }
}

#[test]
fn compose_file_linked_from_the_app_root_keeps_the_app_as_project() {
    let app = TestApp::new("shop");
    app.write("deploy/prod.yml", "services:\n  web:\n    image: nginx\n");
    std::os::unix::fs::symlink("deploy/prod.yml", app.path.join("docker-compose.yml")).unwrap();

    let project = app.read(Some("docker-compose.yml"), &[]).unwrap();

    assert_eq!(project.services[0].name.as_str(), "web");
    assert_eq!(project.project_name(), "shop");
    assert_eq!(project.name_source(), ProjectNameSource::Derived);
}

#[test]
fn app_names_that_normalise_to_another_name_are_declared() {
    // `Shop`, `sh.op` and `shop` all become project `shop`; only the app
    // literally called `shop` may use it unchecked.
    for source in [compose_source, single_image_source] {
        for app in ["Shop", "sh.op"] {
            assert_eq!(
                source(app),
                ("shop".to_string(), ProjectNameSource::Declared),
                "app {app:?}"
            );
        }
    }
}
