use super::ContainerState;
use crate::engine::fixture::{inspect_json, logged, logging_engine, ID, ID_B};
use crate::engine::ContainerId;

#[test]
fn list_filters_by_app_label_and_keeps_full_ids() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), &format!("printf '%s\\n' {ID} {ID_B}"));

    let ids = engine.list("myapp").unwrap();

    assert_eq!(
        logged(&log),
        ["ps -a -q --no-trunc --filter label=riku.app=myapp"]
    );
    assert_eq!(
        ids,
        [
            ContainerId::try_from(ID).unwrap(),
            ContainerId::try_from(ID_B).unwrap()
        ]
    );
}

#[test]
fn list_refuses_an_app_that_could_alter_the_filter() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    for app in ["x,label=a", "../x", "-x", ""] {
        assert!(engine.list(app).is_err(), "accepted {app:?}");
    }
    assert!(
        logged(&log).is_empty(),
        "engine was run: {:?}",
        logged(&log)
    );
}

#[test]
fn list_refuses_engine_output_that_is_not_an_id() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "echo --all");

    let err = engine.list("myapp").unwrap_err().to_string();

    assert!(err.contains("container id"), "unexpected error: {err}");
}

/// Batch inspect fails (a container is gone), single inspect works for A and
/// fails for B. `ps` answers the fresh listing with `listed`.
fn racing_engine(
    dir: &std::path::Path,
    listed: &str,
) -> (crate::engine::Engine, std::path::PathBuf) {
    let json_a = inspect_json(ID, r#"{"Status":"running"}"#, "{}");
    logging_engine(
        dir,
        &format!(
            "case \"$1\" in\n\
             ps) printf '%s\\n' {listed};;\n\
             inspect) if [ \"$#\" -gt 4 ] || [ \"$4\" = {ID_B} ]; then echo gone >&2; exit 1; fi; echo '{json_a}';;\n\
             esac"
        ),
    )
}

fn both() -> [ContainerId; 2] {
    [
        ContainerId::try_from(ID).unwrap(),
        ContainerId::try_from(ID_B).unwrap(),
    ]
}

#[test]
fn inspect_drops_a_container_removed_during_the_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = racing_engine(dir.path(), ID);

    let infos = engine.inspect(&both()).unwrap();

    assert_eq!(infos.len(), 1, "got {infos:?}");
    assert_eq!(infos[0].id.as_str(), ID);
}

/// Every inspect fails. `ps` lists `ID` only when asked without a label
/// filter, and fails when `ps_fails` is set.
fn failing_inspect_engine(
    dir: &std::path::Path,
    ps_fails: bool,
) -> (crate::engine::Engine, std::path::PathBuf) {
    let ps = if ps_fails {
        "echo daemon down >&2; exit 1".to_string()
    } else {
        format!("case \"$*\" in *label*) ;; *) echo {ID};; esac")
    };
    logging_engine(
        dir,
        &format!(
            "case \"$1\" in\n\
             ps) {ps};;\n\
             inspect) echo gone >&2; exit 1;;\n\
             esac"
        ),
    )
}

#[test]
fn a_short_id_that_fails_is_an_error_while_a_container_has_that_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = failing_inspect_engine(dir.path(), false);
    let short = ContainerId::try_from(&ID[..12]).unwrap();

    let err = engine.inspect(&[short]).unwrap_err().to_string();

    assert!(err.contains("gone"), "unexpected error: {err}");
}

#[test]
fn a_failing_container_riku_did_not_label_is_not_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = failing_inspect_engine(dir.path(), false);

    let err = engine.inspect(&both()).unwrap_err().to_string();

    assert!(err.contains("gone"), "unexpected error: {err}");
}

#[test]
fn a_failing_listing_turns_into_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = failing_inspect_engine(dir.path(), true);

    let err = engine.inspect(&both()).unwrap_err().to_string();

    assert!(
        err.contains("listing containers also failed"),
        "unexpected error: {err}"
    );
}

#[test]
fn inspect_fails_when_a_container_that_still_exists_cannot_be_read() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = racing_engine(dir.path(), &format!("{ID} {ID_B}"));

    let err = engine.inspect(&both()).unwrap_err().to_string();

    assert!(err.contains("gone"), "unexpected error: {err}");
}

#[test]
fn inspect_asks_for_containers_by_full_id_and_skips_the_engine_for_none() {
    let dir = tempfile::tempdir().unwrap();
    let json = inspect_json(ID, r#"{"Status":"running"}"#, "{}").replace('\n', " ");
    let (engine, log) = logging_engine(dir.path(), &format!("echo '{json}'"));

    assert!(engine.inspect(&[]).unwrap().is_empty());
    assert!(logged(&log).is_empty(), "ran the engine for no ids");

    let infos = engine
        .inspect(&[ContainerId::try_from(ID).unwrap()])
        .unwrap();

    assert_eq!(logged(&log), [format!("inspect --type container {ID}")]);
    assert_eq!(infos[0].state, ContainerState::Running);
}
