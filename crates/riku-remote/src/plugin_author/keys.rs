//! `plugins keygen` and `plugins sign`: author keys and bundle signatures.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use anyhow::{bail, Result};
use riku_bundle::signing::Keypair;
use riku_bundle::PluginManifest;
use riku_util::{display, write_atomic};

use super::Program;

const KEY_FILE_MODE: u32 = 0o600;
const MANIFEST_FILE: &str = "riku-plugin.toml";

fn is_signature_line(line: &str) -> bool {
    match line.trim_start().strip_prefix("signature") {
        Some(rest) => rest.trim_start().starts_with('='),
        None => false,
    }
}

/// Sets the top-level `signature = "..."` key in a bundle's manifest. The line
/// is inserted among the top-level keys (before the first `[table]`), never
/// appended at the end where TOML would nest it inside the last table.
#[allow(clippy::question_mark)]
fn write_manifest_signature(dir: &Path, signature: &str) -> Result<()> {
    let path = dir.join(MANIFEST_FILE);
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => return Err(e.into()),
    };
    let sig_line = format!("signature = \"{signature}\"\n");

    let mut out = String::new();
    let mut inserted = false;
    for line in existing.lines() {
        if is_signature_line(line) {
            continue;
        }
        if !inserted && line.trim_start().starts_with('[') {
            out.push_str(&sig_line);
            inserted = true;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !inserted {
        out.push_str(&sig_line);
    }
    write_atomic(&path, out.as_bytes())
}

/// Creates `path` with mode 0600 from the start, so the secret is never
/// readable by others, and fails if the path appeared since the caller looked.
#[allow(clippy::question_mark)]
fn write_secret(path: &Path, secret_hex: &str) -> Result<()> {
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(KEY_FILE_MODE)
        .open(path)
    {
        Ok(file) => file,
        Err(e) => return Err(e.into()),
    };
    match writeln!(file, "{secret_hex}") {
        Ok(()) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// `plugins keygen --out <file>`. Writes the secret key to `out` with mode
/// 0600 and prints the public key.
#[allow(clippy::question_mark)]
pub fn cmd_plugins_keygen(program: Program, out: &str) -> Result<()> {
    let path = Path::new(out);
    if path.exists() {
        bail!("'{out}' already exists, choose another --out path");
    }
    let keypair = Keypair::generate();
    if let Err(e) = write_secret(path, &keypair.secret_hex()) {
        return Err(e);
    }

    display::success(&format!("Secret key written to {out} (keep it private)."));
    display::section("Public key (share this so servers can trust you)");
    println!("  {}", keypair.public_hex());
    display::note(&format!(
        "Trust it on a server with: {} <name> <pubkey>",
        program.trust_add()
    ));
    Ok(())
}

/// `plugins sign <bundle> --key <file>`. Signs the entry file and records the
/// signature in the bundle's manifest.
#[allow(clippy::question_mark)]
pub fn cmd_plugins_sign(bundle: &str, key_file: &str) -> Result<()> {
    let secret = match fs::read_to_string(key_file) {
        Ok(text) => text,
        Err(e) => return Err(e.into()),
    };
    let keypair = match Keypair::from_secret_hex(&secret) {
        Ok(keypair) => keypair,
        Err(e) => return Err(e),
    };
    let dir = Path::new(bundle);
    let manifest = match PluginManifest::from_dir(dir) {
        Ok(manifest) => manifest,
        Err(e) => return Err(e),
    };
    let entry = match fs::read(manifest.entry_path(dir)) {
        Ok(bytes) => bytes,
        Err(e) => return Err(e.into()),
    };

    if let Err(e) = write_manifest_signature(dir, &keypair.sign_hex(&entry)) {
        return Err(e);
    }
    display::success(&format!("Signed {} ({})", manifest.name, manifest.entry));
    Ok(())
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
