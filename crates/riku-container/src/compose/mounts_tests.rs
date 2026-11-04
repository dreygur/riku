use crate::compose::test_support::project_of;
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

fn volume_of(app: &str, key: &str) -> MountSource {
    let text =
        format!("services:\n  db:\n    image: postgres:16\n    volumes: [\"{key}:/data\"]\nvolumes:\n  {key}:\n");
    let project = project_of(&text).unwrap();
    let mounts = project.engine_mounts(app, &project.services[0]).unwrap();
    mounts[0].source.clone()
}

#[test]
fn engine_mounts_qualify_volume_keys_with_the_app_name() {
    let project = project_of(WITH_PGDATA).unwrap();

    let mounts = project
        .engine_mounts("myapp", &project.services[0])
        .unwrap();

    assert_eq!(
        mounts,
        vec![
            Mount {
                source: MountSource::Volume("myapp.pgdata".into()),
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
fn apps_whose_names_and_keys_share_underscores_get_different_volumes() {
    let shop = volume_of("shop", "db_data");
    let shop_db = volume_of("shop_db", "data");

    assert_ne!(
        shop, shop_db,
        "app shop and app shop_db would share a volume"
    );
}

#[test]
fn engine_mounts_refuse_an_app_name_the_engine_cannot_take() {
    let project = project_of(WITH_PGDATA).unwrap();

    assert!(project.engine_mounts("-bad", &project.services[0]).is_err());
}
