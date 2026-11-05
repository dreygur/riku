use super::event_container;
use crate::engine::detect::EngineBinary;
use crate::engine::fake::fake_engine;
use crate::engine::fixture::{logged, logging_engine, ID};
use crate::engine::{ContainerId, Engine};

#[test]
fn event_id_comes_from_actor_or_top_level() {
    let docker = format!(
        r#"{{"Type":"container","Action":"die","Actor":{{"ID":"{ID}","Attributes":{{"name":"myapp-web-1"}}}}}}"#
    );
    let podman = format!(r#"{{"ID":"{ID}","Status":"died","Type":"container"}}"#);
    let older = format!(r#"{{"id":"{ID}"}}"#);
    let expected = ContainerId::try_from(ID).unwrap();

    assert_eq!(event_container(&docker), Some(expected.clone()));
    assert_eq!(event_container(&podman), Some(expected.clone()));
    assert_eq!(event_container(&older), Some(expected));
}

#[test]
fn malformed_event_lines_are_ignored() {
    assert_eq!(event_container("not json"), None);
    assert_eq!(event_container(r#"{"Type":"container"}"#), None);
    assert_eq!(event_container(r#"{"Actor":{"ID":"--rm"}}"#), None);
}

#[test]
fn list_riku_selects_by_app_label() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), &format!("printf '%s\\n' {ID}"));

    let ids = engine.list_riku().unwrap();

    assert_eq!(
        logged(&log),
        ["ps -a -q --no-trunc --filter label=riku.app"]
    );
    assert_eq!(ids, [ContainerId::try_from(ID).unwrap()]);
}

#[test]
fn events_command_filters_riku_containers() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "");

    let command = engine.events_command();

    assert_eq!(command.get_program(), engine.program().as_os_str());
    let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy()).collect();
    assert_eq!(
        args,
        [
            "events",
            "--format",
            "{{json .}}",
            "--filter",
            "type=container",
            "--filter",
            "label=riku.app",
            "--filter",
            "event=start",
            "--filter",
            "event=die",
            "--filter",
            "event=health_status",
            "--filter",
            "event=destroy",
        ]
    );
}

#[test]
fn has_events_follows_the_probe() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "");
    assert!(engine.has_events());

    let unsupported_dir = tempfile::tempdir().unwrap();
    let script = concat!(
        "case \"$1 $2\" in \"events --help\") exit 1;; esac\n",
        "case \"$2\" in --help) exit 0;; esac\n",
        "case \"$1\" in version) echo '{\"Client\":{\"Version\":\"1\"}}'; exit 0;; esac\n",
    );
    let program = fake_engine(unsupported_dir.path(), "engine", script);
    let binary = EngineBinary {
        program,
        binary_name: "engine".to_string(),
    };
    let unsupported = Engine::open(binary, Vec::new()).unwrap();
    assert!(!unsupported.has_events());
}
