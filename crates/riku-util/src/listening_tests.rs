use super::*;

const FIXTURE: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 1 1 0000000000000000 100 0 0 10 0
   1: 0100007F:1538 00000000:0000 0A 00000000:00000000 00:00000000 00000000   112        0 2 1 0000000000000000 100 0 0 10 0
   2: 0F02000A:0016 0A02000A:D4F2 01 00000000:00000000 02:000A7B9C 00000000     0        0 3 4 0000000000000000 20 4 29 10 -1
";

#[test]
fn proc_net_tcp_parser_reads_listen_rows_only() {
    let ports = listen_ports_in(FIXTURE);

    assert_eq!(ports, BTreeSet::from([22, 5432]));
}

#[test]
fn malformed_rows_are_skipped() {
    let ports = listen_ports_in("header\n   0: garbage\n   1: 00000000:ZZZZ 00000000:0000 0A\n");

    assert!(ports.is_empty(), "{ports:?}");
}
