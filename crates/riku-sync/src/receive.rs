//! Reads the sender's zstd compressed tar into staging.
//!
//! Security model: the stream is hostile. Entry names are taken as raw text and
//! checked as [`RelPath`]s; the tar is never unpacked by the tar library. An
//! entry must be a regular file that the plan asked for, seen once, and is
//! written with at most its manifest size plus one byte read. Anything else
//! aborts the whole transfer.
//!
//! Bounds, because the tar library reads GNU long name, long link, pax and
//! sparse headers whole into memory before any check here runs, and the
//! manifest sizes are as hostile as the stream:
//!
//! - Decompressed header bytes (header blocks, padding, long names, end blocks)
//!   draw from a budget computed from the requested path lengths alone, see
//!   [`header_budget`]. It does not depend on any declared size.
//! - Body bytes are allowed only while a body copy runs, and then only up to
//!   that entry's declared size plus one.
//! - Compressed input is capped at the decompressed total plus
//!   [`COMPRESSED_MARGIN`] and a 1/64 proportional allowance. zstd stores
//!   incompressible data at a cost of 3 bytes per 128 KiB block, so 1/64 is far
//!   above any real expansion; the margin covers frame headers. This stops
//!   streams of empty or skippable frames, which produce no output.
//! - After the archive ends, the rest of the stream gets a small separate
//!   allowance ([`TRAILER_COMPRESSED`], [`TRAILER_DECOMPRESSED`]) and the zstd
//!   frame must end cleanly, so a cut stream is an error.
//! - The zstd window is limited to [`WINDOW_LOG_MAX`].

use crate::capped::{Budget, CappedReader};
use crate::staging::Staging;
use crate::wire::RelPath;
use std::collections::{HashMap, HashSet};
use std::io::{self, Read};

const BLOCK: u64 = 512;
/// Names this long or longer are written by the sender as an extra GNU long
/// name entry. The real limit is 100 bytes including the NUL, so this is
/// slightly generous.
const LONG_NAME_AT: u64 = 100;
const END_BLOCKS: u64 = 2 * BLOCK;
const COMPRESSED_MARGIN: u64 = 64 * 1024;
const TRAILER_COMPRESSED: u64 = 64 * 1024;
/// Padding to a 10 KiB tar record, rounded up.
const TRAILER_DECOMPRESSED: u64 = 16 * 1024;
/// `write_bundle` compresses at level 3, which uses a 2 MiB window (log 21).
/// 23 leaves headroom for other zstd encoders and caps decoder memory at 8 MiB.
const WINDOW_LOG_MAX: u32 = 23;

/// Header block, up to 511 bytes of data padding, and for a long name an extra
/// header block plus the name rounded up to whole blocks.
fn entry_allowance(path_len: u64) -> u64 {
    let base = BLOCK + (BLOCK - 1);
    if path_len < LONG_NAME_AT {
        return base;
    }
    base + BLOCK + (path_len + 1).div_ceil(BLOCK) * BLOCK
}

fn header_budget(wanted: &HashMap<&str, u64>) -> u64 {
    wanted.keys().fold(END_BLOCKS, |sum, path| {
        sum.saturating_add(entry_allowance(path.len() as u64))
    })
}

fn compressed_budget(header: u64, wanted: &HashMap<&str, u64>) -> u64 {
    let total = wanted.values().fold(header, |sum, size| {
        sum.saturating_add(size.saturating_add(1))
    });
    total
        .saturating_add(total / 64)
        .saturating_add(COMPRESSED_MARGIN)
}

/// Writes every file in `bundle` into `staging`. `wanted` maps each requested
/// path to its manifest size; every one must arrive.
pub(crate) fn receive_bundle(
    staging: &Staging,
    wanted: &HashMap<&str, u64>,
    bundle: impl Read,
) -> anyhow::Result<()> {
    let header = header_budget(wanted);
    let compressed = Budget::new(compressed_budget(header, wanted));
    let decompressed = Budget::new(header);
    let mut decoder = match zstd::Decoder::new(CappedReader::new(bundle, compressed.clone())) {
        Ok(decoder) => decoder,
        Err(e) => return Err(anyhow::anyhow!("bundle is not a zstd stream: {e}")),
    };
    if let Err(e) = decoder.window_log_max(WINDOW_LOG_MAX) {
        return Err(anyhow::anyhow!("cannot limit the zstd window: {e}"));
    }
    let mut archive = tar::Archive::new(CappedReader::new(decoder, decompressed.clone()));
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
            if let Err(e) = store_entry(staging, wanted, &mut arrived, &decompressed, entry) {
                return Err(e);
            }
        }
    }
    if let Some(missing) = wanted.keys().find(|path| !arrived.contains(**path)) {
        return Err(anyhow::anyhow!(
            "bundle is missing requested file {missing:?}"
        ));
    }
    decompressed.set(TRAILER_DECOMPRESSED);
    compressed.set(TRAILER_COMPRESSED);
    drain_trailer(archive.into_inner())
}

fn store_entry(
    staging: &Staging,
    wanted: &HashMap<&str, u64>,
    arrived: &mut HashSet<String>,
    decompressed: &Budget,
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
    decompressed.allow(size.saturating_add(1));
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
