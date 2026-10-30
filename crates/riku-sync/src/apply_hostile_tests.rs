//! A hostile or broken sender: each case must be refused with live unchanged
//! and no staging directory left.

use crate::bundle::write_bundle;
use crate::scan::scan;
use crate::testkit::*;
use crate::wire::{Entry, EntryKind, TransferPlan};
use std::os::unix::fs::symlink;

#[test]
fn path_with_dotdot_in_the_stream_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("ok.txt", "abc")]);
    let bundle = raw_bundle(&[("../escape.txt", b"abc")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["ok.txt"]), bundle);

    assert!(err.contains("escape.txt"), "error was: {err}");
}

#[test]
fn dotdot_path_delivered_in_a_long_name_header_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("ok.txt", "abc")]);
    let bundle = bundle_from(|builder| {
        let name = tar::EntryType::GNULongName;
        append_raw(builder, name, "././@LongLink", "", b"../escape.txt\0");
        append_raw(builder, tar::EntryType::Regular, "short", "", b"abc");
    });

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["ok.txt"]), bundle);

    assert!(err.contains("escape.txt"), "error was: {err}");
}

#[test]
fn long_name_header_claiming_a_huge_body_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("a.txt", "a")]);
    let bundle = bundle_from(|builder| {
        let zeros = vec![0u8; 4 << 20];
        let name = tar::EntryType::GNULongName;
        append_raw(builder, name, "././@LongLink", "", &zeros);
        append_raw(builder, tar::EntryType::Regular, "a.txt", "", b"a");
    });

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(err.contains("larger than"), "error was: {err}");
}

#[test]
fn hard_link_entry_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("a.txt", "a")]);
    let bundle = bundle_from(|builder| {
        let link = tar::EntryType::Link;
        append_raw(builder, link, "a.txt", "/etc/passwd", b"");
    });

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(err.contains("not a regular file"), "error was: {err}");
}

#[test]
fn duplicate_entry_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("a.txt", "a")]);
    let bundle = raw_bundle(&[("a.txt", b"a"), ("a.txt", b"a")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(err.contains("twice"), "error was: {err}");
}

#[test]
fn file_under_a_symlinked_dir_is_refused() {
    let f = fixture();
    let outside = tempfile::tempdir().unwrap();
    let manifest = manifest_of(vec![
        Entry {
            path: rel("lib"),
            kind: EntryKind::Symlink {
                target: outside.path().to_string_lossy().into_owned(),
            },
        },
        file_entry("lib/passwd", "abc"),
    ]);
    let bundle = raw_bundle(&[("lib/passwd", b"abc")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["lib/passwd"]), bundle);

    assert!(err.contains("lib"), "error was: {err}");
    assert!(
        names_in(outside.path()).is_empty(),
        "wrote through the symlink"
    );
}

#[test]
fn live_file_behind_a_live_symlink_is_not_copied() {
    let f = fixture();
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "secret", "s");
    symlink(outside.path(), f.live.join("lib")).unwrap();
    let manifest = manifest_of(vec![dir_entry("lib"), file_entry("lib/secret", "s")]);

    let err = assert_refused_untouched(&f, &manifest, &TransferPlan::default(), raw_bundle(&[]));

    assert!(err.contains("lib/secret"), "error was: {err}");
}

#[test]
fn oversized_file_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("big.txt", "abc")]);
    let bundle = raw_bundle(&[("big.txt", b"0123456789")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["big.txt"]), bundle);

    assert!(err.contains("big.txt"), "error was: {err}");
}

#[test]
fn unrequested_path_in_bundle_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("a.txt", "a")]);
    let bundle = raw_bundle(&[("a.txt", b"a"), ("extra.txt", b"x")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(err.contains("extra.txt"), "error was: {err}");
}

#[test]
fn hash_mismatch_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("a.txt", "abc")]);
    let bundle = raw_bundle(&[("a.txt", b"abd")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(
        err.contains("a.txt") && err.contains("sha256"),
        "error was: {err}"
    );
}

#[test]
fn stream_cut_short_is_refused() {
    let src = tempfile::tempdir().unwrap();
    let body = |seed: u32| {
        (0..6000u32)
            .map(|i| format!("{:x}", i.wrapping_mul(seed) ^ seed))
            .collect::<String>()
    };
    put(src.path(), "one.txt", &body(2654435761));
    put(src.path(), "two.txt", &body(40503));
    let manifest = scan(src.path()).unwrap();
    let plan = plan_needing(&["one.txt", "two.txt"]);
    let mut whole = Vec::new();
    write_bundle(src.path(), &plan.need, &mut whole).unwrap();

    for cut in [whole.len() / 2, whole.len() - 1] {
        let f = fixture();
        assert_refused_untouched(&f, &manifest, &plan, whole[..cut].to_vec());
    }
}

#[test]
fn unchecked_manifest_with_an_unlisted_parent_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("sub/x.txt", "x")]);
    let bundle = raw_bundle(&[("sub/x.txt", b"x")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["sub/x.txt"]), bundle);

    assert!(err.contains("sub"), "error was: {err}");
}

#[test]
fn malformed_sha256_is_refused_plainly() {
    let f = fixture();
    let manifest = manifest_of(vec![Entry {
        path: rel("a.txt"),
        kind: EntryKind::File {
            size: 1,
            sha256: "not-hex".to_string(),
            mode: 0o644,
        },
    }]);
    let bundle = raw_bundle(&[("a.txt", b"a")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(err.contains("malformed sha256"), "error was: {err}");
}

#[test]
fn unchecked_manifest_with_an_unlistable_directory_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![Entry {
        path: rel("locked"),
        kind: EntryKind::Dir { mode: 0o300 },
    }]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&[]), raw_bundle(&[]));

    assert!(
        err.contains("locked") && err.contains("300"),
        "error was: {err}"
    );
}
