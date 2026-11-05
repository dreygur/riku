//! Test doubles: a script engine that records argv, and a parsed compose project.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use riku_container::compose::{default_compose_file, read_project, ComposeProject};
use riku_container::engine::{Engine, EngineBinary};

/// An engine whose `/bin/sh` script answers the probe, appends each other
/// command line to a log, then runs `behaviour`. Returns the engine and the log.
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
    (Engine::open(binary, Vec::new()).unwrap(), log)
}

pub fn logged(log: &Path) -> Vec<String> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// Write `compose` into `<dir>/<app>/compose.yaml` and read it as a project.
pub fn project_in(dir: &Path, app: &str, compose: &str) -> (PathBuf, ComposeProject) {
    let app_path = dir.join(app);
    fs::create_dir_all(&app_path).unwrap();
    fs::write(app_path.join("compose.yaml"), compose).unwrap();
    let file = default_compose_file(&app_path).unwrap();
    let project = read_project(&app_path, &file, &BTreeMap::new()).unwrap();
    (app_path, project)
}
