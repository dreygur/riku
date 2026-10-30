use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use super::ended_message;

#[test]
fn ssh_status_255_names_the_dropped_connection() {
    let status = ExitStatus::from_raw(255 << 8);
    assert_eq!(
        ended_message("riku@edge.example", &status),
        "the ssh connection to riku@edge.example dropped or could not be made"
    );
}

#[test]
fn other_statuses_keep_the_exit_status_message() {
    let status = ExitStatus::from_raw(3 << 8);
    assert_eq!(
        ended_message("riku@edge.example", &status),
        "the remote dashboard ended (exit status: 3)"
    );
}
