use super::*;
use crate::compose::test_support::project_of;
use crate::types::ServiceName;

fn service(name: &str) -> ServiceName {
    ServiceName::try_from(name).unwrap()
}

fn routed_error(text: &str) -> String {
    format!("{:#}", routed_http(&project_of(text).unwrap()).unwrap_err())
}

fn scale_error(text: &str, name: &str, copies: u32) -> String {
    let project = project_of(text).unwrap();
    format!(
        "{:#}",
        check_scale(&project, &service(name), copies).expect_err("scale was accepted")
    )
}

const STORAGE_AND_WEB: &str = "
services:
  web:
    image: app
    labels: {riku.http: \"8080\"}
    ports: [\"8080:80\"]
  db:
    image: postgres
    ports: [\"5432:5432\"]
    volumes: [\"pgdata:/var/lib/postgresql/data\"]
volumes:
  pgdata:
";

#[test]
fn single_published_port_is_routed() {
    let project = project_of(
        "
services:
  web: {image: app, ports: [\"8080:80\"]}
  worker: {image: app}
",
    )
    .unwrap();

    let routed = routed_http(&project).unwrap().expect("one port is routed");
    assert_eq!(routed.service, service("web"));
    assert_eq!(routed.container_port, 80);
}

#[test]
fn no_published_port_means_no_route() {
    let project = project_of("services:\n  worker: {image: app}\n").unwrap();
    assert!(routed_http(&project).unwrap().is_none());
}

#[test]
fn ambiguous_routing_without_label_is_refused() {
    let message = routed_error(
        "
services:
  web: {image: app, ports: [\"8080:80\"]}
  db: {image: postgres, ports: [\"5432:5432\"]}
",
    );
    assert!(
        message.contains("web: 8080")
            && message.contains("db: 5432")
            && message.contains("riku.http"),
        "{message}"
    );
}

#[test]
fn http_label_picks_the_routed_port() {
    let project = project_of(
        "
services:
  web:
    image: app
    labels: {riku.http: \"8080\"}
    ports: [\"8080:80\", \"9090:9090\"]
",
    )
    .unwrap();

    assert_eq!(routed_http(&project).unwrap().unwrap().container_port, 80);
}

#[test]
fn http_label_that_matches_no_published_port_is_refused() {
    let message = routed_error(
        "
services:
  web:
    image: app
    labels: {riku.http: \"7070\"}
    ports: [\"8080:80\"]
",
    );
    assert!(
        message.contains("web") && message.contains("7070"),
        "{message}"
    );
}

#[test]
fn two_services_with_the_http_label_are_refused() {
    let message = routed_error(
        "
services:
  web: {image: app, labels: {riku.http: \"8080\"}, ports: [\"8080:80\"]}
  api: {image: app, labels: {riku.http: \"9090\"}, ports: [\"9090:90\"]}
",
    );
    assert!(
        message.contains("web") && message.contains("api"),
        "{message}"
    );
}

#[test]
fn storage_service_cannot_scale() {
    let message = scale_error(STORAGE_AND_WEB, "db", 2);
    assert!(
        message.contains("storage services cannot scale") && message.contains("pgdata"),
        "{message}"
    );
}

#[test]
fn read_only_mount_does_not_block_scaling() {
    let project = project_of(
        "
services:
  web:
    image: app
    ports: [\"80\"]
    volumes: [\"./conf:/etc/app:ro\"]
",
    )
    .unwrap();

    check_scale(&project, &service("web"), 3).expect("a read only mount does not hold state");
}

#[test]
fn non_http_port_blocks_scaling() {
    let message = scale_error(
        "
services:
  web:
    image: app
    labels: {riku.http: \"8080\"}
    ports: [\"8080:80\", \"9000:9000\"]
",
        "web",
        2,
    );
    assert!(
        message.contains("9000") && message.contains("not routed HTTP"),
        "{message}"
    );
}

#[test]
fn routed_service_with_fixed_host_port_can_scale() {
    let project = project_of("services:\n  web: {image: app, ports: [\"8080:80\"]}\n").unwrap();
    check_scale(&project, &service("web"), 3)
        .expect("phase 3 gives each copy of the routed service its own host port");
}

#[test]
fn zero_copies_and_unknown_services_are_refused() {
    let project = project_of(STORAGE_AND_WEB).unwrap();
    assert!(check_scale(&project, &service("web"), 0).is_err());
    assert!(check_scale(&project, &service("ghost"), 1).is_err());
}

#[test]
fn only_routed_web_overlaps() {
    let project = project_of(
        "
services:
  web: {image: app, ports: [\"8080:80\"]}
  worker: {image: app}
  db:
    image: postgres
    volumes: [\"pgdata:/var/lib/postgresql/data\"]
volumes:
  pgdata:
",
    )
    .unwrap();

    assert_eq!(
        replace_order(&project, &service("web")),
        ReplaceOrder::Overlap
    );
    assert_eq!(
        replace_order(&project, &service("worker")),
        ReplaceOrder::StopFirst
    );
    assert_eq!(
        replace_order(&project, &service("db")),
        ReplaceOrder::StopFirst
    );
}
