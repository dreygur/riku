use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use super::*;

#[test]
fn only_links_into_nginx_root_are_legacy_and_app_named_ones_are_only_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    let sites = &paths.nginx_sites_enabled;
    fs::create_dir_all(sites).unwrap();
    let relative_target = Path::new("..").join(paths.nginx_root.strip_prefix(tmp.path()).unwrap());
    let relative = sites.join("blog.conf");
    symlink(relative_target.join("blog.conf"), &relative).unwrap();
    let admin_made = sites.join("shop.conf");
    symlink("/etc/nginx/sites-available/shop", &admin_made).unwrap();
    symlink("/etc/nginx/sites-available/default", sites.join("default")).unwrap();

    assert_eq!(legacy_site_links(&paths), vec![relative]);
    assert_eq!(app_named_links(&paths), vec![admin_made]);
}
