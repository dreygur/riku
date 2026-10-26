use super::*;

fn env(value: &str) -> HashMap<String, String> {
    HashMap::from([(HTTPS_ONLY_KEY.to_string(), value.to_string())])
}

#[test]
fn https_only_needs_an_explicit_yes() {
    for value in ["1", "true", " TRUE ", "yes", "on"] {
        assert!(
            https_only(&env(value)),
            "{value:?} should turn HTTPS only on"
        );
    }
    for value in ["", "0", "false", "no", "off", "enabled"] {
        assert!(
            !https_only(&env(value)),
            "{value:?} should leave HTTPS only off"
        );
    }
    assert!(!https_only(&HashMap::new()), "unset should be off");
}

#[test]
fn warns_on_unrecognized_value_only() {
    let warning = https_only_warning(&env("enabled")).expect("enabled should warn");
    assert!(warning.contains("RIKU_HTTPS_ONLY=enabled"), "{warning}");

    assert_eq!(https_only_warning(&env("false")), None);
    assert_eq!(https_only_warning(&HashMap::new()), None);
}
