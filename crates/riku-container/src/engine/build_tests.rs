use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::engine::fixture::{logged, logging_engine};
use crate::engine::BuildSpec;

fn spec(context: &str, dockerfile: Option<&str>) -> BuildSpec {
    BuildSpec {
        context: PathBuf::from(context),
        dockerfile: dockerfile.map(PathBuf::from),
        tag: "myapp-web:1".into(),
        args: BTreeMap::new(),
        platform: None,
    }
}

fn refused(spec: &BuildSpec) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");
    let err = engine.build(spec).unwrap_err().to_string();
    assert!(
        logged(&log).is_empty(),
        "engine was called: {:?}",
        logged(&log)
    );
    err
}

#[test]
fn build_joins_a_relative_dockerfile_onto_the_context() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");
    let mut build = spec("/srv/app", Some("docker/Dockerfile.prod"));
    build.args = BTreeMap::from([("RUBY_VERSION".to_string(), "3.3".to_string())]);
    build.platform = Some("linux/amd64".into());

    engine.build(&build).unwrap();

    assert_eq!(
        logged(&log),
        ["build -t myapp-web:1 -f /srv/app/docker/Dockerfile.prod --build-arg RUBY_VERSION=3.3 --platform linux/amd64 /srv/app"]
    );
}

#[test]
fn build_accepts_an_absolute_dockerfile_inside_the_context() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    engine
        .build(&spec("/srv/app", Some("/srv/app/Dockerfile")))
        .unwrap();

    assert_eq!(
        logged(&log),
        ["build -t myapp-web:1 -f /srv/app/Dockerfile /srv/app"]
    );
}

#[test]
fn build_refuses_a_relative_or_flag_like_context() {
    for context in ["app", "--output=/etc"] {
        let err = refused(&spec(context, None));
        assert!(err.contains(context), "{err}");
    }
}

#[test]
fn build_refuses_a_dockerfile_that_leaves_the_context() {
    for dockerfile in [
        "../../etc/passwd",
        "a/../../b",
        "/etc/passwd",
        "/srv/app2/Dockerfile",
    ] {
        let err = refused(&spec("/srv/app", Some(dockerfile)));
        assert!(err.contains(dockerfile), "{err}");
    }
}

#[test]
fn build_refuses_an_argument_name_that_would_change_the_value() {
    let mut build = spec("/srv/app", None);
    build.args.insert("A=B".into(), "c".into());

    let err = refused(&build);

    assert!(err.contains("A=B"), "{err}");
}
