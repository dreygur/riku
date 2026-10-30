use super::parse_stats;
use crate::engine::fixture::{logged, logging_engine};
use crate::engine::ContainerId;

const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Captured from `docker stats --no-stream --format json` (docker 29.8.2) for a
/// running alpine container. The container id and name were replaced.
const DOCKER_LINE: &str = r#"{"BlockIO":"0B / 0B","CPUPerc":"0.00%","Container":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","ID":"0123456789ab","MemPerc":"0.01%","MemUsage":"552KiB / 7.817GiB","Name":"myapp-web-2","NetIO":"1.13kB / 126B","PIDs":"1"}"#;

#[test]
fn docker_capture_prefers_the_full_container_id() {
    let stats = parse_stats(&format!("{DOCKER_LINE}\n")).unwrap();

    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].id, ID);
    assert_eq!(stats[0].cpu_percent, 0.0);
    assert_eq!(stats[0].memory_bytes, 565_248);
}

#[test]
fn reads_one_object_per_line() {
    let out = "{\"ID\":\"abc\",\"CPUPerc\":\"0.50%\",\"MemUsage\":\"21.5MiB / 7.6GiB\"}\n\
               {\"ID\":\"def\",\"CPUPerc\":\"12.25%\",\"MemUsage\":\"1GiB / 7.6GiB\"}\n";

    let stats = parse_stats(out).unwrap();

    assert_eq!(stats[0].id, "abc");
    assert_eq!(
        (stats[0].cpu_percent, stats[0].memory_bytes),
        (0.5, 22_544_384)
    );
    assert_eq!(stats[1].memory_bytes, 1_073_741_824);
}

/// Written from the podman documentation, not captured: podman prints a JSON
/// array with lower case keys and decimal units.
#[test]
fn reads_a_json_array_with_lower_case_keys() {
    let out = r#"[{"id":"abc","cpu_percent":"0.50%","mem_usage":"21.5MB / 8GB"}]"#;

    let s = &parse_stats(out).unwrap()[0];

    assert_eq!(s.id, "abc");
    assert_eq!((s.cpu_percent, s.memory_bytes), (0.5, 21_500_000));
}

#[test]
fn no_output_means_no_stats() {
    assert!(parse_stats("").unwrap().is_empty());
    assert!(parse_stats("\n\n").unwrap().is_empty());
}

#[test]
fn malformed_fields_are_errors_naming_the_field() {
    for (out, field) in [
        (r#"{"CPUPerc":"1%","MemUsage":"1MiB / 2GiB"}"#, "ID"),
        (r#"{"ID":"a","MemUsage":"1MiB / 2GiB"}"#, "CPUPerc"),
        (
            r#"{"ID":"a","CPUPerc":"--","MemUsage":"1MiB / 2GiB"}"#,
            "CPUPerc",
        ),
        (r#"{"ID":"a","CPUPerc":"1%"}"#, "MemUsage"),
        (
            r#"{"ID":"a","CPUPerc":"1%","MemUsage":"lots / 2GiB"}"#,
            "MemUsage",
        ),
        (
            r#"{"ID":"a","CPUPerc":"1%","MemUsage":"1XiB / 2GiB"}"#,
            "MemUsage",
        ),
        (
            r#"{"ID":"a","CPUPerc":"1%","MemUsage":"-1MiB / 2GiB"}"#,
            "MemUsage",
        ),
        ("nonsense", "JSON"),
    ] {
        let err = parse_stats(out).unwrap_err().to_string();
        assert!(
            err.contains(field),
            "expected {field:?} in {err:?} for {out}"
        );
    }
}

#[test]
fn stats_samples_once_and_skips_the_engine_for_no_ids() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), &format!("echo '{DOCKER_LINE}'"));

    assert!(engine.stats(&[]).unwrap().is_empty());
    assert!(logged(&log).is_empty(), "ran the engine for no ids");

    let stats = engine.stats(&[ContainerId::try_from(ID).unwrap()]).unwrap();

    assert_eq!(
        logged(&log),
        [format!("stats --no-stream --format json {ID}")]
    );
    assert_eq!(stats[0].memory_bytes, 565_248);
}
