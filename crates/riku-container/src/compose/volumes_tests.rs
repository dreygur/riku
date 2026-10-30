use super::*;
use crate::types::MountSource;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    app: TempDir,
    vars: BTreeMap<String, String>,
    volumes: BTreeSet<String>,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture {
            app: TempDir::new().unwrap(),
            vars: BTreeMap::new(),
            volumes: ["pgdata".to_string()].into(),
        }
    }

    fn parse(&self, value: Value) -> anyhow::Result<Vec<Mount>> {
        let ctx = ServiceContext {
            app_path: self.app.path(),
            compose_dir: self.app.path(),
            vars: &self.vars,
            declared_volumes: &self.volumes,
        };
        parse_volumes(&ctx, &value)
    }
}

fn mount(source: MountSource, target: &str, read_only: bool) -> Mount {
    Mount {
        source,
        target: Path::new(target).to_path_buf(),
        read_only,
    }
}

#[test]
fn short_and_long_forms() {
    let fx = Fixture::new();
    let app = fx.app.path();
    let mounts = fx
        .parse(json!([
            "pgdata:/var/lib/postgresql/data",
            "./conf:/etc/app:ro",
            "/scratch",
            {"type": "bind", "source": "./static", "target": "/srv", "read_only": true},
            {"type": "volume", "source": "pgdata", "target": "/more"},
        ]))
        .unwrap();
    assert_eq!(
        mounts,
        vec![
            mount(
                MountSource::Volume("pgdata".into()),
                "/var/lib/postgresql/data",
                false
            ),
            mount(MountSource::Bind(app.join("conf")), "/etc/app", true),
            mount(MountSource::Anonymous, "/scratch", false),
            mount(MountSource::Bind(app.join("static")), "/srv", true),
            mount(MountSource::Volume("pgdata".into()), "/more", false),
        ]
    );
}

#[test]
fn bind_source_may_be_missing_but_may_not_leave_the_app() {
    let fx = Fixture::new();
    assert!(fx.parse(json!(["./not-created-yet:/x"])).is_ok());
    let inside_abs = format!("{}/data:/x", fx.app.path().display());
    assert!(fx.parse(json!([inside_abs])).is_ok());

    for source in [
        "../other",
        "./a/../../other",
        "/etc",
        "/var/run/docker.sock",
    ] {
        let err = fx
            .parse(json!([format!("{source}:/x")]))
            .unwrap_err()
            .to_string();
        assert!(err.contains(source), "{source}: {err}");
    }
    assert!(fx
        .parse(json!([{"type": "bind", "source": "../other", "target": "/x"}]))
        .is_err());
}

#[test]
fn bind_path_never_resolves_to_a_sibling_with_the_same_prefix() {
    let fx = Fixture::new();
    let sibling = PathBuf::from(format!("{}-evil/data", fx.app.path().display()));
    let err = fx.parse(json!([format!("{}:/x", sibling.display())]));
    assert!(err.is_err());
}

#[test]
fn refuses_undeclared_volumes_other_types_and_unsafe_text() {
    let fx = Fixture::new();
    for value in [
        json!(["cache:/x"]),
        json!([{"type": "tmpfs", "target": "/tmp"}]),
        json!([{"type": "volume", "source": "pgdata", "target": "/x", "volume": {"nocopy": true}}]),
        json!(["pgdata:relative"]),
        json!(["pgdata:/x:ro,z"]),
        json!(["pgdata:/x:ro:extra"]),
        json!([{"type": "bind", "source": "./a,b", "target": "/x"}]),
        json!([{"type": "bind", "target": "/x"}]),
        json!(["-v:/x"]),
    ] {
        assert!(fx.parse(value.clone()).is_err(), "{value} was accepted");
    }
}
