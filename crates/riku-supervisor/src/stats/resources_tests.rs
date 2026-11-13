use super::*;

fn entry(pid: u32, ppid: u32, cpu_time_ms: u64, memory_bytes: u64) -> ProcEntry {
    ProcEntry {
        pid,
        ppid,
        usage: ResourceUsage {
            cpu_time_ms,
            memory_bytes,
        },
    }
}

#[test]
fn parse_stat_counts_fields_from_the_last_paren() {
    let line = "4242 (my (odd) app) S 4200 4242 4242 0 -1 4194560 120 0 0 0 \
                30 12 5 3 20 0 1 0 98765 12345678 480 18446744073709551615";

    let fields = parse_stat(line).expect("a well formed stat line parses");

    assert_eq!(
        fields,
        StatFields {
            ppid: 4200,
            cpu_ticks: 30 + 12 + 5 + 3,
            rss_pages: 480,
        }
    );
}

#[test]
fn parse_stat_refuses_a_truncated_line() {
    assert_eq!(parse_stat("4242 (sh) S 1 4242"), None);
}

/// `sh -c <cmd>` is the worker's pid; the real app is its child and may
/// fork again. All of them count, an unrelated process does not.
#[test]
fn tree_usage_sums_the_wrapper_and_every_descendant() {
    let table = ProcessTable::from_entries(vec![
        entry(1, 0, 999, 999),
        entry(100, 1, 10, 2_000_000),
        entry(101, 100, 400, 19_600_000),
        entry(102, 101, 50, 1_000_000),
        entry(200, 1, 7, 7_000_000),
    ]);

    assert_eq!(
        table.tree_usage(100),
        ResourceUsage {
            cpu_time_ms: 460,
            memory_bytes: 22_600_000,
        }
    );
}

/// A pid reused between two reads can make the snapshot loop; the walk
/// must still end.
#[test]
fn tree_usage_ends_on_a_parent_loop() {
    let table = ProcessTable::from_entries(vec![entry(5, 9, 1, 1), entry(9, 5, 2, 2)]);

    assert_eq!(
        table.tree_usage(5),
        ResourceUsage {
            cpu_time_ms: 3,
            memory_bytes: 3,
        }
    );
}
