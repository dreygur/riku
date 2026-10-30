use crate::compose::test_support::{project_of, TestApp};
use crate::types::{Mount, MountSource};
use std::path::PathBuf;

const WITH_PGDATA: &str = "
services:
  db:
    image: postgres:16
    volumes: [\"pgdata:/var/lib/postgresql/data\", \"/scratch\"]
volumes:
  pgdata:
";

#[test]
fn engine_mounts_qualify_volume_keys_with_the_project_name() {
    let project = project_of(WITH_PGDATA).unwrap();

    let mounts = project.engine_mounts(&project.services[0]).unwrap();

    assert_eq!(
        mounts,
        vec![
            Mount {
                source: MountSource::Volume("shop_pgdata".into()),
                target: PathBuf::from("/var/lib/postgresql/data"),
                read_only: false,
            },
            Mount {
                source: MountSource::Anonymous,
                target: PathBuf::from("/scratch"),
                read_only: false,
            },
        ]
    );
}

#[test]
fn engine_mounts_refuse_an_unconfirmed_declared_project() {
    let app = TestApp::new("myapp");
    app.write("compose.yaml", &format!("name: shop\n{WITH_PGDATA}"));
    let mut project = app.read(None, &[]).unwrap();
    let service = project.services[0].clone();

    let refusal = match project.engine_mounts(&service) {
        Ok(mounts) => panic!("unconfirmed declared project got mounts {mounts:?}"),
        Err(e) => format!("{e:#}"),
    };
    assert!(
        refusal.contains("shop"),
        "refusal should name the project: {refusal}"
    );

    project.confirm_declared_name();
    let mounts = project.engine_mounts(&service).unwrap();
    assert_eq!(mounts[0].source, MountSource::Volume("shop_pgdata".into()));
}
