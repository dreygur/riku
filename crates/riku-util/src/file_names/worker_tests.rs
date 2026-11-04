use super::*;

#[test]
fn apps_whose_names_join_alike_get_distinct_files() {
    let shop = WorkerName::new("shop", "api-web", 1);
    let shop_api = WorkerName::new("shop-api", "web", 1);

    assert_ne!(
        shop.file_name(),
        shop_api.file_name(),
        "with `-` both were shop-api-web-1.toml"
    );
    for worker in [shop, shop_api] {
        assert_eq!(
            WorkerName::parse(&worker.to_string()),
            Some(worker.clone()),
            "{worker} must read back as its own app, kind and ordinal"
        );
    }
}
