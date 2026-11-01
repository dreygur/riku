use super::*;

#[test]
fn container_name_joins_app_service_and_copy() {
    let web = ServiceName::try_from("web").unwrap();
    let name = ContainerName::new("myapp", &web, CopyIndex::new(1).unwrap()).unwrap();
    assert_eq!(name.as_str(), "myapp-web-1");
}

#[test]
fn release_container_name_is_app_dash_release_and_validates_the_app() {
    assert_eq!(
        ContainerName::for_release("myapp").unwrap().as_str(),
        "myapp-release"
    );
    assert!(ContainerName::for_release("-myapp").is_err());
}

#[test]
fn container_name_rejects_unvalidated_app() {
    let web = ServiceName::try_from("web").unwrap();
    assert!(ContainerName::new("../myapp", &web, CopyIndex::new(1).unwrap()).is_err());
}

#[test]
fn container_name_rejects_app_the_engine_would_refuse() {
    let web = ServiceName::try_from("web").unwrap();
    let copy = CopyIndex::new(1).unwrap();
    for app in ["_myapp", ".myapp", "-myapp", "caf\u{e9}"] {
        assert!(
            riku_util::validate_app_name(app).is_ok_and(|v| v == app),
            "{app:?} is no longer accepted by validate_app_name, pick another example"
        );
        assert!(
            ContainerName::new(app, &web, copy).is_err(),
            "accepted {app:?}"
        );
    }
}

#[test]
fn service_name_rejects_engine_unsafe_characters() {
    for bad in ["", "-web", "web/../x", "we b", "web;rm"] {
        assert!(ServiceName::try_from(bad).is_err(), "accepted {bad:?}");
    }
    for good in ["web", "db_1", "api.v2"] {
        assert!(ServiceName::try_from(good).is_ok(), "rejected {good:?}");
    }
}

#[test]
fn service_name_is_at_most_63_characters() {
    assert!(ServiceName::try_from("a".repeat(63).as_str()).is_ok());
    assert!(ServiceName::try_from("a".repeat(64).as_str()).is_err());
}

#[test]
fn copy_index_starts_at_one() {
    assert!(CopyIndex::new(0).is_none());
}

#[test]
fn network_is_one_per_app() {
    assert_eq!(NetworkName::for_app("myapp").as_str(), "riku-myapp");
}
