//! Runs the driver against a real docker. Ignored by default; run with
//! `RIKU_TEST_DOCKER=$(which docker) cargo test -p riku-container -- --ignored docker_`.

use std::path::PathBuf;
use std::process::Command;

use crate::engine::detect::EngineBinary;
use crate::engine::Engine;
use crate::types::{ContainerName, CopyIndex, ServiceName};

fn docker(args: &[&str]) {
    let program = std::env::var("RIKU_TEST_DOCKER").expect("set RIKU_TEST_DOCKER");
    let status = Command::new(program).args(args).output().unwrap().status;
    assert!(status.success(), "docker {args:?} failed");
}

fn engine() -> Engine {
    let program = PathBuf::from(std::env::var("RIKU_TEST_DOCKER").expect("set RIKU_TEST_DOCKER"));
    Engine::open(
        EngineBinary {
            program,
            binary_name: "docker".to_string(),
        },
        Vec::new(),
    )
    .unwrap()
}

#[test]
#[ignore = "needs a docker engine"]
fn docker_owner_of_list_project_and_volume_labels() {
    let name = "rkdockertest-web-1";
    let volume = "rkdockertest_data";
    docker(&["rm", "-f", name]);
    docker(&["volume", "rm", "-f", volume]);
    docker(&["volume", "create", "--label", "riku.app=shop", volume]);
    docker(&[
        "run",
        "-d",
        "--name",
        name,
        "--label",
        "riku.app=shop",
        "--label",
        "com.docker.compose.project=rkdockertest",
        "alpine",
        "sleep",
        "60",
    ]);
    let engine = engine();
    let service = ServiceName::try_from("web").unwrap();
    let container = ContainerName::new("rkdockertest", &service, CopyIndex::new(1).unwrap());

    let owner = engine.owner_of(&container.unwrap());
    let project = engine.list_project("rkdockertest");
    let labels = engine.volume_labels(volume);
    docker(&["rm", "-f", name]);
    docker(&["volume", "rm", "-f", volume]);

    assert_eq!(owner.unwrap().as_deref(), Some("shop"));
    assert_eq!(project.unwrap().len(), 1);
    assert_eq!(labels.unwrap()["riku.app"], "shop");
}
