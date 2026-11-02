//! A mock `systemctl` for the reload tests.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// One `systemctl show` answer: start, exit, status, ActiveState.
pub type Run = (u64, u64, u32, &'static str);

pub fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let script = dir.join(name);
    std::fs::write(&script, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

/// Pin a modification time in whole seconds. The mock shell compares times
/// with `-nt`, which on macOS only looks at seconds.
fn set_mtime(path: &Path, epoch_secs: u64) {
    let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(epoch_secs);
    std::fs::File::open(path)
        .and_then(|handle| handle.set_modified(when))
        .unwrap();
}

fn lines(runs: &[Run]) -> String {
    runs.iter()
        .map(|(start, exit, status, active)| format!("{start} {exit} {status} {active}\n"))
        .collect()
}

/// A `systemctl` that answers from `phases`. Phase 0 answers until the nginx
/// config directory is touched; each touch moves to the next phase when there
/// is one, and is ignored when there is none. Within a phase the Nth call
/// gets line N, repeating the last line once they run out. The mock puts the
/// directory's time back after it sees a touch, so the next touch shows too.
pub fn mock_systemctl(dir: &Path, phases: &[&[Run]]) -> PathBuf {
    for (i, phase) in phases.iter().enumerate() {
        std::fs::write(dir.join(format!("phase_{i}")), lines(phase)).unwrap();
    }
    let watched = dir.join(".riku/nginx");
    std::fs::create_dir_all(&watched).unwrap();
    std::fs::write(dir.join("marker"), "").unwrap();
    set_mtime(&watched, 1_000_000_000);
    set_mtime(&dir.join("marker"), 1_100_000_000);
    let body = format!(
        "dir={dir}\n\
         phase=$(cat \"$dir/phase\" 2>/dev/null || echo 0)\n\
         calls=$(cat \"$dir/calls\" 2>/dev/null || echo 0)\n\
         if [ \"$dir/.riku/nginx\" -nt \"$dir/marker\" ]; then\n\
             touch -t 200101010000 \"$dir/.riku/nginx\"\n\
             next=$((phase + 1))\n\
             if [ -s \"$dir/phase_$next\" ]; then\n\
                 phase=$next\n\
                 echo $phase > \"$dir/phase\"\n\
                 calls=0\n\
             fi\n\
         fi\n\
         calls=$((calls + 1))\n\
         echo $calls > \"$dir/calls\"\n\
         seq=\"$dir/phase_$phase\"\n\
         line=$(sed -n \"${{calls}}p\" \"$seq\")\n\
         [ -n \"$line\" ] || line=$(tail -n 1 \"$seq\")\n\
         set -- $line\n\
         echo ExecMainStartTimestampMonotonic=$1\n\
         echo ExecMainExitTimestampMonotonic=$2\n\
         echo ExecMainStatus=$3\n\
         echo ActiveState=$4\n\
         echo LoadState=loaded\n",
        dir = dir.display(),
    );
    write_script(dir, "mock_systemctl", &body)
}
