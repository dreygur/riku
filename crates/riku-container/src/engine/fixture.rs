//! A fake engine that records the argv of every command after the probe.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::detect::EngineBinary;
use super::fake::fake_engine;
use super::{Engine, RunSpec};
use crate::types::{
    labels, ContainerName, CopyIndex, NetworkName, ServiceName, StopSignal, DEFAULT_STOP_GRACE,
};

/// Open an engine whose `/bin/sh` script answers the probe, appends each other
/// command line to a log, then runs `behaviour`. Returns the engine and the log path.
pub fn logging_engine(dir: &Path, behaviour: &str) -> (Engine, PathBuf) {
    let log = dir.join("argv.log");
    let script = format!(
        "case \"$2\" in --help) exit 0;; esac\n\
         case \"$1\" in version) echo '{{\"Client\":{{\"Version\":\"1\"}}}}'; exit 0;; esac\n\
         printf '%s\\n' \"$*\" >> '{}'\n\
         {behaviour}",
        log.display()
    );
    let binary = EngineBinary {
        program: fake_engine(dir, "engine", &script),
        binary_name: "engine".to_string(),
    };
    (Engine::open(binary).unwrap(), log)
}

/// A minimal valid spec for `myapp-web-1`; tests change the fields they care about.
pub fn sample_spec() -> RunSpec {
    let service = ServiceName::try_from("web").unwrap();
    let labels = [
        (labels::APP, "myapp"),
        (labels::SERVICE, "web"),
        (labels::COPY, "1"),
    ];
    RunSpec {
        name: ContainerName::new("myapp", &service, CopyIndex::new(1).unwrap()).unwrap(),
        image: "registry.example/myapp:1".to_string(),
        network: NetworkName::for_app("myapp"),
        alias: service,
        labels: labels
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        env: BTreeMap::new(),
        ports: Vec::new(),
        mounts: Vec::new(),
        entrypoint: None,
        command: None,
        working_dir: None,
        user: None,
        stop_signal: StopSignal::default(),
        stop_grace: DEFAULT_STOP_GRACE,
        healthcheck: None,
    }
}

/// Two full container ids for tests that need one or two containers.
pub const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const ID_B: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

/// Image id every [`inspect_json`] container reports.
pub const IMAGE_ID: &str =
    "sha256:be2656061b536c1883c4034790e21d0a09c5aaaf51968b89368a3c0371ea2c9a";

/// `inspect` output for one container named `myapp-web-2` running image
/// `shop/web:1.4`, with the given `State` and `Labels` JSON.
pub fn inspect_json(id: &str, state: &str, labels: &str) -> String {
    format!(
        r#"[{{"Id":"{id}","Name":"/myapp-web-2","Image":"{IMAGE_ID}","Config":{{"Image":"shop/web:1.4","Labels":{labels}}},"State":{state}}}]"#
    )
}

/// The recorded command lines, one per engine call.
pub fn logged(log: &Path) -> Vec<String> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}
