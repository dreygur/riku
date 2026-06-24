//! Shared-secret authentication for the mutating control-plane routes.
//!
//! Security model: the read-only `/health` and `/metrics*` routes stay open
//! on `127.0.0.1` with permissive CORS (status data only, low sensitivity).
//! Mutating routes (deploy/restart/stop/destroy/create) are gated by a
//! random token persisted to `control_token_file` with `0600` permissions.
//! The dashboard's server-side proxy reads that file directly and attaches
//! it as `Authorization: Bearer <token>`; the token is never sent to a
//! browser. This stops a malicious page running in a local browser (CORS
//! `Any` + no auth would otherwise let any origin trigger destructive
//! requests via DNS rebinding) from acting on the control plane.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const TOKEN_BYTES: usize = 32;

/// Load the control-plane token from disk, generating and persisting a new
/// one on first use. The token file is created with `0600` permissions so
/// only the owning user can read it.
pub fn load_or_create_token(token_file: &Path) -> io::Result<String> {
    match fs::read_to_string(token_file) {
        Ok(existing) => {
            let trimmed = existing.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let token = generate_token();

    if let Some(parent) = token_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(token_file, &token)?;
    fs::set_permissions(token_file, fs::Permissions::from_mode(0o600))?;

    Ok(token)
}

/// Generate a 256-bit token from the OS CSPRNG, hex-encoded (lowercase).
fn generate_token() -> String {
    use rand::rngs::OsRng;
    use rand::RngCore;

    let mut bytes = [0u8; TOKEN_BYTES];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Constant-time string comparison, re-exported from the shared util crate so
/// existing `health::auth::constant_time_eq` call-sites keep working.
pub use crate::util::secure::constant_time_eq;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn generates_and_persists_token_with_owner_only_perms() {
        let dir = tempfile::tempdir().unwrap();
        let token_file = dir.path().join("control.token");

        let first = load_or_create_token(&token_file).unwrap();
        assert_eq!(first.len(), TOKEN_BYTES * 2);

        let perms = fs::metadata(&token_file).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);

        let second = load_or_create_token(&token_file).unwrap();
        assert_eq!(first, second, "token must be stable across reloads");
    }

    #[test]
    fn constant_time_eq_matches_normal_equality() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("short", "longer-string"));
        assert!(constant_time_eq("", ""));
    }
}
