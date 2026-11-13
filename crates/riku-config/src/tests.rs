use super::*;
use std::path::Path;

/// Helper: build RikuPaths with a known home and root, no env vars involved.
fn paths_with_root(root: &str, home: &str) -> RikuPaths {
    RikuPaths::from_dirs(PathBuf::from(root), &PathBuf::from(home))
}

#[test]
fn default_paths_use_home_dot_riku() {
    let home = "/home/testuser";
    let paths = paths_with_root(&format!("{home}/.riku"), home);
    assert_eq!(paths.riku_root, Path::new("/home/testuser/.riku"));
}

#[test]
fn all_subdirectory_paths_are_relative_to_riku_root() {
    let root = "/srv/riku";
    let home = "/home/riku";
    let p = paths_with_root(root, home);

    assert_eq!(p.app_root, Path::new("/srv/riku/apps"));
    assert_eq!(p.data_root, Path::new("/srv/riku/data"));
    assert_eq!(p.env_root, Path::new("/srv/riku/envs"));
    assert_eq!(p.git_root, Path::new("/srv/riku/repos"));
    assert_eq!(p.log_root, Path::new("/srv/riku/logs"));
    assert_eq!(p.nginx_root, Path::new("/srv/riku/nginx"));
    assert_eq!(p.cache_root, Path::new("/srv/riku/cache"));
    assert_eq!(p.workers_root, Path::new("/srv/riku/workers"));
    assert_eq!(
        p.workers_available,
        Path::new("/srv/riku/workers-available")
    );
    assert_eq!(p.workers_enabled, Path::new("/srv/riku/workers-enabled"));
    assert_eq!(p.acme_cert_links, Path::new("/srv/riku/acme"));
    assert_eq!(p.acme_root, Path::new("/var/lib/riku/acme"));
    assert_eq!(p.static_root, Path::new("/var/lib/riku/static"));
    assert_eq!(p.plugin_root, Path::new("/srv/riku/plugins"));
}

#[test]
fn custom_root_parameter_works() {
    let p = paths_with_root("/opt/custom-riku", "/home/deploy");
    assert_eq!(p.riku_root, Path::new("/opt/custom-riku"));
    assert_eq!(p.app_root, Path::new("/opt/custom-riku/apps"));
    assert_eq!(p.git_root, Path::new("/opt/custom-riku/repos"));
}

#[test]
fn acme_sh_root_is_riku_acme_root_else_home_acme_sh() {
    let home = Path::new("/home/bob");

    assert_eq!(
        super::acme_sh_root(None, home),
        Path::new("/home/bob/.acme.sh")
    );
    assert_eq!(
        super::acme_sh_root(Some("/custom/acme".to_string()), home),
        Path::new("/custom/acme")
    );
}

#[test]
fn from_env_uses_home() {
    let paths = RikuPaths::from_env().expect("HOME is set in the test environment");
    assert!(paths.riku_root.is_absolute() || env::var("RIKU_ROOT").is_ok());
}

#[test]
fn root_dirs_fall_back_to_home_dot_riku() {
    let (root, home) = root_dirs(Some("/home/riku".into()), None).unwrap();
    assert_eq!(home, Path::new("/home/riku"));
    assert_eq!(root, Path::new("/home/riku/.riku"));
}

#[test]
fn root_dirs_reject_relative_or_climbing_paths() {
    let cases = [
        (Some("relative/home"), None, "HOME"),
        (Some("/home/riku"), Some(".riku"), "RIKU_ROOT"),
        (Some("/home/riku"), Some("/srv/../etc"), "RIKU_ROOT"),
    ];
    for (home, root, var) in cases {
        let err = root_dirs(home.map(String::from), root.map(String::from))
            .expect_err("unsafe root must be rejected");
        assert!(
            err.to_string().contains(var),
            "HOME={home:?} RIKU_ROOT={root:?}: error should name {var}, got: {err}"
        );
    }
}

/// conf.d wherever it exists (Debian, Ubuntu, RHEL, nginx.org), so those
/// hosts never change; Alpine's http.d only when conf.d is missing.
#[test]
fn the_include_goes_to_conf_d_unless_only_alpines_http_d_exists() {
    let conf_d = Path::new("/etc/nginx/conf.d/riku.conf");
    let http_d = Path::new("/etc/nginx/http.d/riku.conf");
    let with = |dirs: &'static [&'static str]| {
        crate::nginx_host::host_include_file(move |dir: &Path| {
            dirs.iter().any(|d| Path::new(d) == dir)
        })
    };

    assert_eq!(with(&["/etc/nginx/conf.d"]), conf_d);
    assert_eq!(with(&["/etc/nginx/conf.d", "/etc/nginx/http.d"]), conf_d);
    assert_eq!(with(&["/etc/nginx/http.d"]), http_d);
    assert_eq!(with(&[]), conf_d, "no nginx yet: init warns about conf.d");
    assert_eq!(NginxHostPaths::system(), NginxHostPaths::resolve(None));
}

#[test]
fn for_tests_keeps_nginx_host_paths_inside_the_test_root() {
    let p = RikuPaths::for_tests(Path::new("/tmp/riku-case"));
    assert!(
        p.nginx_include_file.starts_with("/tmp/riku-case"),
        "a test must never touch the machine's real nginx: {:?}",
        p.nginx_include_file
    );
}
