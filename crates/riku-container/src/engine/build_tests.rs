use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::engine::fixture::{logged, logging_engine};
use crate::engine::BuildSpec;
use riku_util::display::OutputTail;

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
    let err = engine
        .build(spec, &OutputTail::default())
        .unwrap_err()
        .to_string();
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

    engine.build(&build, &OutputTail::default()).unwrap();

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
        .build(
            &spec("/srv/app", Some("/srv/app/Dockerfile")),
            &OutputTail::default(),
        )
        .unwrap();

    assert_eq!(
        logged(&log),
        ["build -t myapp-web:1 -f /srv/app/Dockerfile /srv/app"]
    );
}

#[test]
fn build_output_from_both_streams_goes_through_the_rail_tail() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(
        dir.path(),
        "echo 'Step 2/4 : WORKDIR /app'; echo ' ---> Running in 0b00b8f6' >&2",
    );
    let recent = OutputTail::default();

    engine.build(&spec("/srv/app", None), &recent).unwrap();

    let lines = recent.lines();
    assert!(
        lines.iter().any(|l| l == "Step 2/4 : WORKDIR /app")
            && lines.iter().any(|l| l == " ---> Running in 0b00b8f6"),
        "build output did not reach the tail: {lines:?}"
    );
}

#[test]
fn build_drops_the_legacy_builder_notice_and_keeps_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(
        dir.path(),
        "printf '%s\\n' \
           'DEPRECATED: The legacy builder is deprecated and will be removed in a future release.' \
           '            Install the buildx component to build images with BuildKit:' \
           '            https://docs.docker.com/go/buildx/' \
           '' >&2; \
         echo 'Sending build context to Docker daemon  3.072kB'; \
         echo 'Step 1/4 : FROM python:3.12-slim'",
    );
    let recent = OutputTail::default();

    engine.build(&spec("/srv/app", None), &recent).unwrap();

    assert_eq!(
        recent.lines(),
        [
            "Sending build context to Docker daemon  3.072kB",
            "Step 1/4 : FROM python:3.12-slim"
        ]
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
        "",
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
