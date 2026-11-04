use super::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_parse_procfile_basic() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "web: python app.py").unwrap();
    writeln!(f, "worker: celery -A tasks").unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert_eq!(workers.get("web").unwrap(), "python app.py");
    assert_eq!(workers.get("worker").unwrap(), "celery -A tasks");
}

#[test]
fn test_parse_procfile_comments_and_blanks() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "# This is a comment").unwrap();
    writeln!(f).unwrap();
    writeln!(f, "web: python app.py").unwrap();
    writeln!(f).unwrap();
    writeln!(f, "# Another comment").unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers.get("web").unwrap(), "python app.py");
}

#[test]
fn test_parse_procfile_wsgi_trumps_web() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "web: python app.py").unwrap();
    writeln!(f, "wsgi: app:application").unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert!(!workers.contains_key("web"));
    assert!(workers.contains_key("wsgi"));
}

#[test]
fn test_parse_procfile_missing_file() {
    let result = parse_procfile(Path::new("/nonexistent/Procfile")).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_parse_procfile_cron_valid() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "cron: */5 * * * * /usr/bin/task").unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert!(workers.contains_key("cron"));
}

#[test]
fn test_parse_procfile_cron_invalid_value() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "cron: 60 * * * * /usr/bin/task").unwrap();
    assert!(
        parse_procfile(f.path()).is_err(),
        "a bad schedule must fail the parse"
    );
}

#[test]
fn test_parse_procfile_cron_rejects_hour_24() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "cron: 0 24 * * * /usr/bin/task").unwrap();
    assert!(
        parse_procfile(f.path()).is_err(),
        "a bad schedule must fail the parse"
    );
}

#[test]
fn test_parse_procfile_cron_rejects_weekday_8() {
    // Standard Unix cron allows weekday 0-7 (0 and 7 both Sunday); 8 is
    // out of range and must be rejected by the shared engine.
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "cron: 0 0 * * 8 /usr/bin/task").unwrap();
    assert!(
        parse_procfile(f.path()).is_err(),
        "a bad schedule must fail the parse"
    );
}

#[test]
fn test_parse_procfile_cron_accepts_weekday_7_sunday() {
    // 7 is a legal alias for Sunday in Unix cron: the old regex-based
    // validator wrongly rejected it; the engine accepts it.
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "cron: 0 0 * * 7 /usr/bin/task").unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert!(workers.contains_key("cron"));
}

#[test]
fn test_parse_procfile_cron_accepts_valid_bounds() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "cron: 59 23 31 12 6 /usr/bin/task").unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert!(workers.contains_key("cron"));
}

#[test]
fn test_parse_procfile_cron_accepts_ranges_and_lists() {
    // The old CRON_REGEXP rejected ranges/lists; the engine accepts them.
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "cron: 0 9 * * 1-5 /usr/bin/weekday-task").unwrap();
    writeln!(f, "cron2: 0,15,30,45 * * * * /usr/bin/quarter-hour").unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert!(workers.contains_key("cron"));
    assert!(workers.contains_key("cron2"));
}

#[test]
fn whitespace_and_comments_only_mean_no_processes() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "   \n\t\n# nothing here").unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert!(workers.is_empty(), "got {workers:?}");
}

#[test]
fn rejects_a_line_that_is_not_kind_colon_command() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "web: python app.py").unwrap();
    writeln!(f, "worker python worker.py").unwrap();
    let err = parse_procfile(f.path()).unwrap_err();
    assert!(err.to_string().contains("line 2"), "{err}");
}

#[test]
fn rejects_a_process_kind_that_is_not_a_plain_name() {
    for line in ["../x: rm -rf ~", "foo bar: baz", ": python app.py"] {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "{line}").unwrap();
        assert!(
            parse_procfile(f.path()).is_err(),
            "{line:?} should be rejected"
        );
    }
}

#[test]
fn test_parse_procfile_empty_file() {
    let f = NamedTempFile::new().unwrap();
    let workers = parse_procfile(f.path()).unwrap().unwrap();
    assert!(workers.is_empty());
}
