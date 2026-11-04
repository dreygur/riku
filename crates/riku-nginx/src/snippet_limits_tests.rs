use super::check_limits;
use crate::snippet_grammar::statements;

fn limits(snippet: &str) -> Result<(), String> {
    check_limits(&statements(snippet).expect("test snippet fits the grammar"))
}

#[test]
fn values_at_their_caps_pass_and_one_over_or_unparsed_is_refused_by_directive() {
    let at_caps = "client_max_body_size 1g; keepalive_timeout 1h 60s; proxy_read_timeout 500ms;\n\
                   proxy_buffer_size 1024k; proxy_buffers 64 1m; proxy_busy_buffers_size \"2m\";\n\
                   gzip_comp_level 9; add_header X-Frame-Options \"SAMEORIGIN\" always;";
    assert_eq!(limits(at_caps), Ok(()));

    let refused = [
        ("client_max_body_size 1025m;", "client_max_body_size 1025m"),
        ("client_max_body_size 0;", "client_max_body_size 0"),
        ("proxy_send_timeout 2d;", "proxy_send_timeout 2d"),
        ("client_body_timeout 1m30s;", "not an nginx time"),
        ("proxy_buffers 65 4k;", "proxy_buffers"),
        ("proxy_busy_buffers_size 10mb;", "not an nginx size"),
        ("gzip_comp_level 0;", "gzip_comp_level 0"),
    ];
    for (snippet, named) in refused {
        let reason = limits(snippet).expect_err(snippet);
        assert!(reason.contains(named), "{snippet}: {reason}");
    }

    let long_value = format!("add_header X-Big \"{}\";", "a".repeat(4097));
    let reason = limits(&long_value).expect_err("a 4097 byte header value");
    assert!(reason.contains("add_header X-Big"), "{reason}");

    let many = "add_header X-A b;\n".repeat(33);
    let reason = limits(&many).expect_err("33 add_header lines");
    assert!(reason.contains("33 `add_header`"), "{reason}");
}
