//! Streams that try to make the receiver read or buffer without bound.

use crate::testkit::*;
use crate::wire::{Entry, EntryKind};

const SKIPPABLE_EMPTY_FRAME: [u8; 8] = [0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0];

#[test]
fn a_huge_declared_size_does_not_raise_the_header_allowance() {
    let f = fixture();
    let manifest = manifest_of(vec![Entry {
        path: rel("big.bin"),
        kind: EntryKind::File {
            size: 1 << 40,
            sha256: "0".repeat(64),
            mode: 0o644,
        },
    }]);
    let bundle = bundle_from(|builder| {
        let zeros = vec![0u8; 4 << 20];
        let name = tar::EntryType::GNULongName;
        append_raw(builder, name, "././@LongLink", "", &zeros);
        append_raw(builder, tar::EntryType::Regular, "big.bin", "", b"x");
    });

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["big.bin"]), bundle);

    assert!(err.contains("larger than"), "error was: {err}");
}

#[test]
fn trailer_limit_stops_frames_after_a_large_valid_archive() {
    let f = fixture();
    let body = "\0".repeat(1 << 20);
    let manifest = manifest_of(vec![file_entry("a.bin", &body)]);
    let mut bundle = raw_bundle(&[("a.bin", body.as_bytes())]);
    for _ in 0..40_000 {
        bundle.extend_from_slice(&SKIPPABLE_EMPTY_FRAME);
    }

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.bin"]), bundle);

    assert!(err.contains("trailer"), "error was: {err}");
}

#[test]
fn endless_empty_frames_after_the_archive_are_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("a.txt", "a")]);
    let mut bundle = raw_bundle(&[("a.txt", b"a")]);
    for _ in 0..20_000 {
        bundle.extend_from_slice(&SKIPPABLE_EMPTY_FRAME);
    }

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(err.contains("larger than"), "error was: {err}");
}
