use super::*;

fn vars(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn plain_braced_and_default_forms_expand() {
    let vars = vars(&[("TAG", "1.4"), ("PORT", "8000"), ("EMPTY", "")]);
    let mut warnings = Vec::new();

    let text = "shop/web:${TAG} $PORT ${EMPTY:-5000} ${HOST:-0.0.0.0}";
    let out = interpolate(text, &vars, &mut warnings).unwrap();

    assert_eq!(out, "shop/web:1.4 8000 5000 0.0.0.0");
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn double_dollar_is_a_literal_dollar() {
    let mut warnings = Vec::new();
    assert_eq!(
        interpolate("a$$b", &BTreeMap::new(), &mut warnings).unwrap(),
        "a$b"
    );
}

#[test]
fn unsupported_forms_are_errors_naming_the_expression() {
    for expr in [
        "${DB:?missing}",
        "${DB?missing}",
        "${DB-x}",
        "${DB:+x}",
        "${DB+x}",
        "${A:-${B}}",
        "${DB",
        "$1",
    ] {
        let mut warnings = Vec::new();
        let err = interpolate(expr, &BTreeMap::new(), &mut warnings)
            .expect_err(expr)
            .to_string();
        assert!(err.contains(expr), "{expr}: {err}");
    }
}

#[test]
fn unset_variable_becomes_empty_and_warns_once() {
    let mut warnings = Vec::new();
    let out = interpolate("[${NOPE}$NOPE]", &BTreeMap::new(), &mut warnings).unwrap();

    assert_eq!(out, "[]");
    assert_eq!(
        warnings,
        ["variable NOPE is not set; using an empty string"]
    );
}
