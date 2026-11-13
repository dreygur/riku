use std::path::Path;

use super::*;

#[test]
fn ssh_keygen_runs_quietly_so_its_output_stays_off_the_rail() {
    let command = keygen_command(Path::new("/home/u/.ssh/id_ed25519"));

    let args: Vec<_> = command.get_args().collect();
    assert!(
        args.contains(&"-q".as_ref()),
        "without -q ssh-keygen prints its randomart, got {args:?}"
    );
    assert!(
        args.contains(&"/home/u/.ssh/id_ed25519".as_ref()),
        "the key path must be passed as given, got {args:?}"
    );
}
