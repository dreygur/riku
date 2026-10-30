//! Sender side: packs the requested files into one zstd compressed tar.

use crate::guard::real_file;
use crate::wire::RelPath;
use std::fs;
use std::io::Write;
use std::path::Path;

const ZSTD_LEVEL: i32 = 3;

/// Writes the files in `need`, read from under `src`, as a tar compressed with
/// zstd. Only regular files reached through real directories are packed; a
/// symlink or special file in `need` is an error rather than a followed link.
pub fn write_bundle(src: &Path, need: &[RelPath], out: impl Write) -> anyhow::Result<()> {
    let encoder = match zstd::Encoder::new(out, ZSTD_LEVEL) {
        Ok(encoder) => encoder,
        Err(e) => return Err(anyhow::anyhow!("cannot start zstd stream: {e}")),
    };
    let mut builder = tar::Builder::new(encoder);
    builder.mode(tar::HeaderMode::Deterministic);
    for path in need {
        let full = match real_file(src, path) {
            Ok(full) => full,
            Err(e) => return Err(e),
        };
        let mut file = match fs::File::open(&full) {
            Ok(file) => file,
            Err(e) => return Err(anyhow::anyhow!("cannot open {}: {e}", path.as_str())),
        };
        if let Err(e) = builder.append_file(path.as_str(), &mut file) {
            return Err(anyhow::anyhow!("cannot pack {}: {e}", path.as_str()));
        }
    }
    let encoder = match builder.into_inner() {
        Ok(encoder) => encoder,
        Err(e) => return Err(anyhow::anyhow!("cannot finish tar stream: {e}")),
    };
    match encoder.finish() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("cannot finish zstd stream: {e}")),
    }
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
