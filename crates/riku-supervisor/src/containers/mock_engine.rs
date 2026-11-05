//! Mock engine test double for supervisor container tests.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use riku_container::engine::{Engine, EngineBinary};

/// Read the argv lines logged by a mock engine.
pub fn logged(log: &Path) -> Vec<String> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// Create a mock engine script in `dir` that logs argv and executes `behaviour`.
pub fn mock_engine(dir: &Path, behaviour: &str) -> (Engine, PathBuf) {
    let log = dir.join("argv.log");
    let script = format!(
        "#!/bin/sh\n\
         case \"$2\" in --help) exit 0;; esac\n\
         case \"$1\" in version) echo '{{\"Client\":{{\"Version\":\"1\"}}}}'; exit 0;; esac\n\
         printf '%s\\n' \"$*\" >> '{}'\n\
         {behaviour}\n",
        log.display()
    );
    let program = dir.join("engine");
    fs::write(&program, script).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    let binary = EngineBinary {
        program,
        binary_name: "engine".to_string(),
    };
    (Engine::open(binary).unwrap(), log)
}

/// Create a mock engine script in `dir` that fails `events --help`.
pub fn mock_engine_without_events(dir: &Path, behaviour: &str) -> (Engine, PathBuf) {
    let log = dir.join("argv.log");
    let script = format!(
        "#!/bin/sh\n\
         case \"$1 $2\" in \"events --help\") exit 1;; esac\n\
         case \"$2\" in --help) exit 0;; esac\n\
         case \"$1\" in version) echo '{{\"Client\":{{\"Version\":\"1\"}}}}'; exit 0;; esac\n\
         printf '%s\\n' \"$*\" >> '{}'\n\
         {behaviour}\n",
        log.display()
    );
    let program = dir.join("engine");
    fs::write(&program, script).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    let binary = EngineBinary {
        program,
        binary_name: "engine".to_string(),
    };
    (Engine::open(binary).unwrap(), log)
}
