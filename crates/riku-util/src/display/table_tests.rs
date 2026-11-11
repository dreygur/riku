use super::*;

#[test]
fn no_headers_renders_nothing() {
    assert_eq!(format_table(&[], &[], 2), "");
}

#[test]
fn columns_widen_to_the_longest_cell() {
    let rows = vec![
        vec!["a-very-long-app-name".to_string(), "ok".to_string()],
        vec!["web".to_string(), "running".to_string()],
    ];
    let table = format_table(&["App", "Status"], &rows, 2);
    assert_eq!(
        table,
        "App                   Status \n\
         --------------------  -------\n\
         a-very-long-app-name  ok     \n\
         web                   running\n"
    );
}

#[test]
fn a_short_row_does_not_panic() {
    let rows = vec![vec!["only-one".to_string()]];
    let table = format_table(&["App", "Status"], &rows, 2);
    assert!(table.ends_with("only-one\n"), "got:\n{table}");
}
