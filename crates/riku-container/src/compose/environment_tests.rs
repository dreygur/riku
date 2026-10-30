use super::*;
use tempfile::TempDir;

#[test]
fn parses_plain_quoted_exported_and_commented_lines() {
    let text = "\
# database
export DB_HOST=db.internal
TAG = 1.4 # pinned
GREETING=\"hello\\nworld\"
RAW='a $b \\n'

EMPTY=
";
    let vars = parse_dotenv(".env", text).unwrap();

    assert_eq!(vars["DB_HOST"], "db.internal");
    assert_eq!(vars["TAG"], "1.4");
    assert_eq!(vars["GREETING"], "hello\nworld");
    assert_eq!(vars["RAW"], "a $b \\n");
    assert_eq!(vars["EMPTY"], "");
    assert_eq!(vars.len(), 5);
}

#[test]
fn quoted_value_may_be_followed_by_a_comment() {
    let vars = parse_dotenv(".env", "A=\"v\" # note\nB='v' # note\nC=\"v#1\"\n").unwrap();
    assert_eq!(vars["A"], "v");
    assert_eq!(vars["B"], "v");
    assert_eq!(vars["C"], "v#1");
    assert!(parse_dotenv(".env", "D=\"v\" trailing\n").is_err());
}

#[test]
fn malformed_line_names_file_and_line() {
    let err = parse_dotenv("/app/.env", "OK=1\n\nnot a variable\n")
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("/app/.env:3:"), "{err}");

    let err = parse_dotenv(".env", "BAD=\"open\n")
        .unwrap_err()
        .to_string();
    assert!(err.starts_with(".env:1: BAD:"), "{err}");
}

#[test]
fn missing_file_is_empty_and_outside_symlink_is_refused() {
    let app = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let dotenv = app.path().join(".env");
    assert!(read_dotenv(app.path(), &dotenv).unwrap().is_empty());

    std::fs::write(outside.path().join("secrets"), "TOKEN=x\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secrets"), &dotenv).unwrap();
    let err = read_dotenv(app.path(), &dotenv).unwrap_err().to_string();
    assert!(err.contains("escapes"), "{err}");
}
