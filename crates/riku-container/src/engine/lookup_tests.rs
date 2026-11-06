use crate::engine::fixture::{inspect_json, logged, logging_engine, ID, ID_B, IMAGE_ID};
use crate::engine::ContainerId;
use crate::types::{ContainerName, CopyIndex, ServiceName};

fn name(app: &str, copy: u32) -> ContainerName {
    let service = ServiceName::try_from("web").unwrap();
    ContainerName::new(app, &service, CopyIndex::new(copy).unwrap()).unwrap()
}

fn engine_listing_one(
    dir: &std::path::Path,
    labels: &str,
) -> (crate::engine::Engine, std::path::PathBuf) {
    let json = inspect_json(ID, r#"{"Status":"running"}"#, labels);
    logging_engine(
        dir,
        &format!("case \"$1\" in ps) echo {ID};; inspect) echo '{json}';; esac"),
    )
}

#[test]
fn owner_of_reports_the_label_the_empty_string_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = engine_listing_one(dir.path(), r#"{"riku.app":"other"}"#);
    assert_eq!(
        engine.owner_of(&name("myapp", 2)).unwrap().as_deref(),
        Some("other")
    );
    assert_eq!(
        logged(&log)[0],
        r"ps -a -q --no-trunc --filter name=^myapp\.web\.2$"
    );

    let unlabelled = tempfile::tempdir().unwrap();
    let (engine, _log) = engine_listing_one(unlabelled.path(), "{}");
    assert_eq!(
        engine.owner_of(&name("myapp", 2)).unwrap().as_deref(),
        Some("")
    );

    let absent = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(absent.path(), "");
    assert_eq!(engine.owner_of(&name("myapp", 2)).unwrap(), None);
    assert_eq!(logged(&log).len(), 1, "inspected with nothing listed");
}

#[test]
fn id_of_reports_the_full_id_of_the_named_container() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = engine_listing_one(dir.path(), "{}");

    let found = engine.id_of(&name("myapp", 2)).unwrap();

    assert_eq!(found.as_ref().map(ContainerId::as_str), Some(ID));
}

#[test]
fn owner_of_escapes_dots_and_ignores_a_container_with_another_name() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = engine_listing_one(dir.path(), r#"{"riku.app":"other"}"#);

    let owner = engine.owner_of(&name("my.app", 2)).unwrap();

    assert_eq!(owner, None, "took myapp.web.2 for my.app.web.2");
    assert_eq!(
        logged(&log)[0],
        r"ps -a -q --no-trunc --filter name=^my\.app\.web\.2$"
    );
}

#[test]
fn project_filter_is_exact() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), &format!("printf '%s\\n' {ID} {ID_B}"));

    let ids = engine.list_project("shop").unwrap();

    assert_eq!(
        logged(&log),
        ["ps -a -q --no-trunc --filter label=com.docker.compose.project=shop"]
    );
    assert_eq!(ids, [ID, ID_B].map(|id| ContainerId::try_from(id).unwrap()));
}

#[test]
fn list_project_refuses_a_name_that_could_alter_the_filter() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    for project in ["shop,label=a", "-shop", ""] {
        assert!(
            engine.list_project(project).is_err(),
            "accepted {project:?}"
        );
    }
    assert!(
        logged(&log).is_empty(),
        "engine was run: {:?}",
        logged(&log)
    );
}

#[test]
fn image_id_is_none_when_absent_and_refuses_an_ambiguous_reference() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), &format!("echo {IMAGE_ID}"));
    assert_eq!(
        engine.image_id("shop/web:1.4").unwrap().as_deref(),
        Some(IMAGE_ID)
    );
    assert_eq!(logged(&log), ["image ls -q --no-trunc shop/web:1.4"]);

    let absent = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(absent.path(), "");
    assert_eq!(engine.image_id("shop/web:1.4").unwrap(), None);
    for image in ["-q", "shop/*", "shop/we?", "shop/[wx]eb"] {
        assert!(engine.image_id(image).is_err(), "accepted {image:?}");
    }

    let several = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(several.path(), "echo sha256:aa; echo sha256:bb");
    let err = engine.image_id("shop/web").unwrap_err().to_string();
    assert!(err.contains("several images"), "{err}");
}
