use std::path::Path;

use super::*;

const OWN: &str = "/etc/nginx/conf.d/riku.conf";

fn dump(files: &[(&str, &str)]) -> String {
    files
        .iter()
        .map(|(path, text)| format!("# configuration file {path}:\n{text}\n"))
        .collect()
}

#[test]
fn catch_all_is_added_only_when_no_other_file_is_the_port_80_default() {
    let own_catch_all = (OWN, CATCH_ALL_SERVER);
    let cases = [
        (
            dump(&[("/etc/nginx/nginx.conf", "http {}"), own_catch_all]),
            true,
        ),
        (
            dump(&[(
                "/etc/nginx/sites-enabled/default",
                "server {\n  listen [::]:80 default_server ipv6only=on;\n}",
            )]),
            false,
        ),
        (
            dump(&[(
                "/etc/nginx/sites-enabled/x",
                "  listen 443 ssl default_server;",
            )]),
            true,
        ),
        (
            dump(&[(
                "/etc/nginx/sites-enabled/x",
                "  # listen 80 default_server;",
            )]),
            true,
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(needs_catch_all(&text, Path::new(OWN)), expected, "{text}");
    }
}
