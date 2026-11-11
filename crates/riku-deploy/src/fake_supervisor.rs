//! A stand-in supervisor for tests: a `sleep` whose pid is in
//! `supervisor.pid`, with that file locked the way a real supervisor keeps
//! it locked. A SIGHUP ends the `sleep`, so a test can see it was sent.

use std::fs::{self, File};
use std::process::{Child, Command};

use crate::config::RikuPaths;

pub(crate) struct FakeSupervisor {
    pub(crate) child: Child,
    _lock: File,
}

impl FakeSupervisor {
    pub(crate) fn start(paths: &RikuPaths) -> FakeSupervisor {
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        fs::create_dir_all(&paths.riku_root).unwrap();
        let pid_file = paths.riku_root.join("supervisor.pid");
        fs::write(&pid_file, child.id().to_string()).unwrap();
        let lock = File::open(&pid_file).unwrap();
        lock.lock().unwrap();
        FakeSupervisor { child, _lock: lock }
    }
}

impl Drop for FakeSupervisor {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
