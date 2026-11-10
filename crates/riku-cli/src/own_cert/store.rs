//! Where an app's own certificate lives and who put it there.
//!
//! The pair uses the files riku keeps certificates in already, in
//! `nginx_root`: `<app>.key`, `<app>.crt` (leaf) and `<app>.fullchain.crt`.
//! The marker `<app>.own-cert` is written last and removed first, so a
//! crash never leaves a marker without a pair that riku would then refuse
//! to replace.
//!
//! Security model: callers pass an app name already validated against
//! `validate_app_name`, so it is a plain file stem. The key is written
//! `0600` from creation; the public files get the usual mode.

#![allow(clippy::question_mark)]

use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use super::bundle::{first_certificate, Bundle};
use crate::config::own_cert::{has_own_cert, OWN_CERT_SUFFIX};
use crate::config::RikuPaths;
use crate::util::{write_atomic, write_private};

const PAIR_SUFFIXES: [&str; 3] = ["key", "crt", "fullchain.crt"];

/// Who made the certificate an app serves.
#[derive(Debug, PartialEq)]
pub(crate) enum Source {
    Own,
    LetsEncrypt,
    SelfSigned,
    None,
}

fn path(paths: &RikuPaths, app: &str, suffix: &str) -> PathBuf {
    paths.nginx_root.join(format!("{app}.{suffix}"))
}

/// The marker wins. Otherwise a full chain file means acme.sh installed it
/// (the self-signed fallback writes only a key and a certificate).
pub(crate) fn source(paths: &RikuPaths, app: &str) -> Source {
    if has_own_cert(paths, app) {
        Source::Own
    } else if path(paths, app, "fullchain.crt").exists() {
        Source::LetsEncrypt
    } else if path(paths, app, "crt").exists() {
        Source::SelfSigned
    } else {
        Source::None
    }
}

/// The leaf certificate riku serves for `app`, if it has one on disk.
pub(crate) fn leaf_pem(paths: &RikuPaths, app: &str) -> Option<String> {
    ["fullchain.crt", "crt"]
        .iter()
        .find_map(|suffix| fs::read_to_string(path(paths, app, suffix)).ok())
        .map(|pem| first_certificate(&pem).to_string())
}

fn write(paths: &RikuPaths, app: &str, suffix: &str, text: &str) -> Result<()> {
    let file = path(paths, app, suffix);
    let written = if suffix == "key" {
        write_private(&file, text.as_bytes())
    } else {
        write_atomic(&file, text.as_bytes())
    };
    match written {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot write {app}.{suffix}: {e:#}")),
    }
}

/// Save the pair, then the marker.
pub(crate) fn install(paths: &RikuPaths, app: &str, bundle: &Bundle) -> Result<()> {
    let files = [
        ("key", bundle.key_pem.as_str()),
        ("crt", first_certificate(&bundle.cert_pem)),
        ("fullchain.crt", bundle.cert_pem.as_str()),
        (OWN_CERT_SUFFIX, ""),
    ];
    for (suffix, text) in files {
        if let Err(e) = write(paths, app, suffix, text) {
            return Err(e);
        }
    }
    Ok(())
}

/// Delete the marker, then the pair.
pub(crate) fn remove(paths: &RikuPaths, app: &str) -> Result<()> {
    for suffix in [OWN_CERT_SUFFIX].into_iter().chain(PAIR_SUFFIXES) {
        let file = path(paths, app, suffix);
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(anyhow!("cannot remove {}: {e}", file.display())),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
