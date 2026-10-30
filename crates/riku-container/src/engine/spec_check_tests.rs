use std::path::PathBuf;

use super::check_spec;
use crate::engine::fixture::sample_spec;
use crate::types::{Mount, MountSource};

fn mount(source: MountSource, target: &str) -> Mount {
    Mount {
        source,
        target: PathBuf::from(target),
        read_only: false,
    }
}

#[test]
fn accepts_the_sample_spec() {
    check_spec(&sample_spec()).unwrap();
}

#[test]
fn refuses_an_image_the_engine_would_read_as_a_flag() {
    let mut spec = sample_spec();
    spec.image = "--privileged".to_string();

    let err = check_spec(&spec).unwrap_err().to_string();

    assert!(err.contains("--privileged"), "{err}");
}

#[test]
fn refuses_label_key_with_equals() {
    let mut spec = sample_spec();
    spec.labels.insert("a=b".into(), "v".into());

    let err = check_spec(&spec).unwrap_err().to_string();

    assert!(err.contains("a=b"), "{err}");
}

#[test]
fn refuses_mounts_that_smuggle_options_or_change_kind() {
    let bad = [
        mount(MountSource::Volume("data:/etc:rw".into()), "/x"),
        mount(MountSource::Volume("data".into()), "/x:ro"),
        mount(MountSource::Bind(PathBuf::from("relative/dir")), "/x"),
        mount(MountSource::Bind(PathBuf::from("/srv/a:b")), "/x"),
        mount(MountSource::Anonymous, "relative"),
    ];
    for m in bad {
        let mut spec = sample_spec();
        spec.mounts = vec![m.clone()];
        assert!(check_spec(&spec).is_err(), "{m:?} was accepted");
    }
}
