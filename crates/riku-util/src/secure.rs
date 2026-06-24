//! Constant-time comparison for secret material (tokens, signatures).

/// Constant-time string equality. A length mismatch returns early (token
/// lengths are fixed, so this leaks nothing secret) while avoiding a panic;
/// equal-length inputs are compared byte-wise without short-circuiting.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;

    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}
