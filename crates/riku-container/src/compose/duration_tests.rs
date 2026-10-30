use super::*;

#[test]
fn combines_units_in_one_value() {
    assert_eq!(parse_duration("1m30s").unwrap(), Duration::from_secs(90));
    assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
    assert_eq!(parse_duration("250ms").unwrap(), Duration::from_millis(250));
    assert_eq!(parse_duration("40us").unwrap(), Duration::from_micros(40));
}

#[test]
fn rejects_values_without_a_unit_or_with_an_unknown_one() {
    for raw in ["", "30", "5d", "s", "1.5s", "1m 30s", "-5s", "10s30"] {
        let err = parse_duration(raw).unwrap_err().to_string();
        assert!(err.contains(&format!("{raw:?}")), "{raw:?}: {err}");
    }
}

#[test]
fn rejects_a_value_that_overflows() {
    assert!(parse_duration("99999999999999999999h").is_err());
}
