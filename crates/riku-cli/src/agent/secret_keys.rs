//! Which config keys look like they hold a secret.
//!
//! A name match is a heuristic: a secret under an unrelated name (`REDIS_URL`
//! with a password inside) is not caught. It decides which changes need a
//! confirm token and which values a readonly agent sees masked.

/// Name fragments that mark a key as secret, compared case-insensitively.
pub const SECRET_MARKERS: [&str; 9] = [
    "SECRET",
    "PASS",
    "PWD",
    "TOKEN",
    "_KEY",
    "AUTH",
    "CREDENTIAL",
    "PRIVATE",
    "DATABASE_URL",
];

/// What a masked value is replaced with.
pub const MASK: &str = "***";

pub fn is_secret(key: &str) -> bool {
    let key = key.to_ascii_uppercase();
    SECRET_MARKERS.iter().any(|marker| key.contains(marker))
}
