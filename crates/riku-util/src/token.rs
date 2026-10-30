//! Random token generation shared by the control plane and the dashboard client.

const TOKEN_BYTES: usize = 32;

/// Generate a 256-bit token from the OS CSPRNG, hex-encoded (lowercase).
pub fn generate_token() -> String {
    use rand::rand_core::UnwrapErr;
    use rand::rngs::SysRng;
    use rand::Rng;

    let mut bytes = [0u8; TOKEN_BYTES];
    UnwrapErr(SysRng).fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
