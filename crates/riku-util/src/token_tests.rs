use super::*;

#[test]
fn token_is_64_lowercase_hex_and_unique() {
    let first = generate_token();
    assert_eq!(first.len(), 64, "token was {first}");
    assert!(
        first
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "token was {first}"
    );
    assert_ne!(first, generate_token());
}
