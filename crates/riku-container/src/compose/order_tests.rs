use crate::compose::test_support::project_of;

fn names(text: &str) -> Vec<String> {
    project_of(text)
        .unwrap()
        .services
        .iter()
        .map(|service| service.name.to_string())
        .collect()
}

#[test]
fn services_start_after_their_dependencies() {
    let order = names(
        "
services:
  worker: {image: app, depends_on: [web]}
  web: {image: app, depends_on: [db]}
  db: {image: postgres}
  cache: {image: redis}
",
    );
    assert_eq!(order, ["cache", "db", "web", "worker"]);
}

#[test]
fn dependency_cycle_is_refused_naming_the_services() {
    let text = "
services:
  web: {image: app, depends_on: [worker]}
  worker: {image: app, depends_on: [web]}
";
    let message = format!("{:#}", project_of(text).unwrap_err());
    assert!(
        message.contains("web") && message.contains("worker"),
        "{message}"
    );
}

#[test]
fn unknown_dependency_is_refused_naming_both_services() {
    let text = "
services:
  web: {image: app, depends_on: [cache]}
";
    let message = format!("{:#}", project_of(text).unwrap_err());
    assert!(
        message.contains("web") && message.contains("cache"),
        "{message}"
    );
}
