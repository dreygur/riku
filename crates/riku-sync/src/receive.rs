//! Reads the sender's zstd compressed tar into staging.
//!
//! Security model: the stream is hostile. Entry names are taken as raw text and
//! checked as [`RelPath`]s; the tar is never unpacked by the tar library. An
//! entry must be a regular file that the plan asked for, seen once, and is
//! written with at most its manifest size plus one byte read. Anything else
//! aborts the whole transfer.
//!
//! Memory and decompression bounds: the tar library reads GNU long name, long
//! link, pax and sparse headers whole into memory before any check here runs, so
//! the decompressed stream is capped at the wanted sizes plus per entry slack
//! ([`ENTRY_SLACK`]) plus end blocks and record padding ([`END_SLACK`]); crossing
//! the cap is an error. The zstd window is limited to [`WINDOW_LOG_MAX`]. The
//! compressed frame must end cleanly, so a cut stream is an error.

use crate::capped::CappedReader;
use crate::staging::Staging;
use crate::wire::RelPath;
use std::collections::{HashMap, HashSet};
use std::io::{self, Read};

/// Header block, data padding and long name slack allowed per wanted file.
const ENTRY_SLACK: u64 = 1536;
/// Two end blocks plus padding to a 10 KiB tar record, rounded up.
const END_SLACK: u64 = 16 * 1024;
/// `write_bundle` compresses at level 3, which uses a 2 MiB window (log 21).
/// 23 leaves headroom for other zstd encoders and caps decoder memory at 8 MiB.
const WINDOW_LOG_MAX: u32 = 23;

/// Writes every file in `bundle` into `staging`. `wanted` maps each requested
/// path to its manifest size; every one must arrive.
pub(crate) fn receive_bundle(
    staging: &Staging,
    wanted: &HashMap<&str, u64>,
    bundle: impl Read,
) -> anyhow::Result<()> {
    let mut decoder = match zstd::Decoder::new(bundle) {
        Ok(decoder) => decoder,
        Err(e) => return Err(anyhow::anyhow!("bundle is not a zstd stream: {e}")),
    };
    if let Err(e) = decoder.window_log_max(WINDOW_LOG_MAX) {
        return Err(anyhow::anyhow!("cannot limit the zstd window: {e}"));
    }
    let payload: u64 = wanted
        .values()
        .fold(0, |sum, size| sum.saturating_add(*size));
    let slack = ENTRY_SLACK
        .saturating_mul(wanted.len() as u64)
        .saturating_add(END_SLACK);
    let capped = CappedReader::new(decoder, payload.saturating_add(slack));
    let mut archive = tar::Archive::new(capped);
    let mut arrived: HashSet<String> = HashSet::new();
    {
        let entries = match archive.entries() {
            Ok(entries) => entries,
            Err(e) => return Err(anyhow::anyhow!("cannot read bundle: {e}")),
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Err(anyhow::anyhow!("bundle stream is damaged: {e}")),
            };
            if let Err(e) = store_entry(staging, wanted, &mut arrived, entry) {
                return Err(e);
            }
        }
    }
    if let Some(missing) = wanted.keys().find(|path| !arrived.contains(**path)) {
        return Err(anyhow::anyhow!(
            "bundle is missing requested file {missing:?}"
        ));
    }
    drain_trailer(archive.into_inner())
}

fn store_entry(
    staging: &Staging,
    wanted: &HashMap<&str, u64>,
    arrived: &mut HashSet<String>,
    mut entry: tar::Entry<impl Read>,
) -> anyhow::Result<()> {
    if entry.header().entry_type() != tar::EntryType::Regular {
        return Err(anyhow::anyhow!(
            "bundle holds an entry that is not a regular file"
        ));
    }
    let name = match entry.path() {
        Ok(name) => match name.to_str() {
            Some(text) => text.to_string(),
            None => return Err(anyhow::anyhow!("bundle entry name is not valid UTF-8")),
        },
        Err(e) => return Err(anyhow::anyhow!("bundle entry has an unreadable name: {e}")),
    };
    let path = match RelPath::try_from(name.as_str()) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    let size = match wanted.get(path.as_str()) {
        Some(size) => *size,
        None => {
            return Err(anyhow::anyhow!(
                "bundle holds {name:?}, which was not requested"
            ))
        }
    };
    if !arrived.insert(name.clone()) {
        return Err(anyhow::anyhow!("bundle holds {name:?} twice"));
    }
    let mut file = match staging.create_file(&path) {
        Ok(file) => file,
        Err(e) => return Err(e),
    };
    let written = match io::copy(&mut (&mut entry).take(size.saturating_add(1)), &mut file) {
        Ok(written) => written,
        Err(e) => return Err(anyhow::anyhow!("cannot store {name:?}: {e}")),
    };
    if written != size {
        return Err(anyhow::anyhow!(
            "{name:?} does not have the {size} bytes the manifest states"
        ));
    }
    Ok(())
}

fn drain_trailer(mut rest: impl Read) -> anyhow::Result<()> {
    match io::copy(&mut rest, &mut io::sink()) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("bundle stream is damaged: {e}")),
    }
}
