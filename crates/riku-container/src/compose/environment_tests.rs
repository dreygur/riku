use super::*;
use tempfile::TempDir;

/// Parse `text` as `.env` with no app ENV, dropping warnings.
fn parse_dotenv(source: &str, text: &str) -> Result<BTreeMap<String, String>> {
    parse_with(source, text, &[])
}

fn parse_with(
    source: &str,
    text: &str,
    app_env: &[(&str, &str)],
) -> Result<BTreeMap<String, String>> {
    let app_env: BTreeMap<String, String> = app_env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    super::parse_dotenv(source, text, &app_env, &mut Vec::new())
}

#[test]
fn bare_key_takes_the_app_env_value_or_stays_unset() {
    let vars = parse_with(
        ".env",
        "FROM_APP\nMISSING\nexport ALSO\n",
        &[("FROM_APP", "a"), ("ALSO", "b")],
    )
    .unwrap();

    assert_eq!(vars.get("FROM_APP").map(String::as_str), Some("a"));
    assert_eq!(vars.get("ALSO").map(String::as_str), Some("b"));
    assert!(
        !vars.contains_key("MISSING"),
        "unset bare key was set: {vars:?}"
    );
}

#[test]
fn quoted_values_may_span_lines() {
    let text = "CERT=\"line one\nline two\" # pem\nNEXT=1\nRAW='a\nb'\n";

    let vars = parse_dotenv(".env", text).unwrap();

    assert_eq!(vars["CERT"], "line one\nline two");
    assert_eq!(vars["RAW"], "a\nb");
    assert_eq!(vars["NEXT"], "1");
}

#[test]
fn unterminated_quote_names_file_and_starting_line() {
    let err = parse_dotenv("/app/.env", "OK=1\nKEY=\"open\nstill open\n")
        .unwrap_err()
        .to_string();

    assert!(err.starts_with("/app/.env:2: KEY:"), "{err}");
}

#[test]
fn doubling_chain_is_refused_at_the_cap() {
    // Each line doubles the one before; unchecked, 30 lines of this reach
    // a terabyte from a file of under 1 KiB.
    let mut text = format!("V0={}\n", "x".repeat(1000));
    for i in 1..13 {
        text.push_str(&format!("V{i}=${{V{prev}}}${{V{prev}}}\n", prev = i - 1));
    }

    let err = match parse_dotenv("/app/.env", &text) {
        Ok(vars) => panic!(
            "doubling chain accepted, last value {} bytes",
            vars["V12"].len()
        ),
        Err(e) => e.to_string(),
    };

    assert!(
        err.starts_with("/app/.env:") && err.contains("1048576 bytes"),
        "{err}"
    );
}

#[test]
fn values_expand_earlier_lines_and_the_app_env_but_not_single_quotes() {
    let text =
        "HOST=db\nURL=postgres://${HOST}/$NAME\nQUOTED=\"${HOST}:5432\"\nLITERAL='${HOST}'\n";

    let vars = parse_with(".env", text, &[("NAME", "shop")]).unwrap();

    assert_eq!(vars["URL"], "postgres://db/shop");
    assert_eq!(vars["QUOTED"], "db:5432");
    assert_eq!(vars["LITERAL"], "${HOST}");
}

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
    let no_env = BTreeMap::new();
    assert!(read_dotenv(app.path(), &dotenv, &no_env, &mut Vec::new())
        .unwrap()
        .is_empty());

    std::fs::write(outside.path().join("secrets"), "TOKEN=x\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secrets"), &dotenv).unwrap();
    let err = read_dotenv(app.path(), &dotenv, &no_env, &mut Vec::new())
        .unwrap_err()
        .to_string();
    assert!(err.contains("escapes"), "{err}");
}
