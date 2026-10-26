use super::*;

// select_template

#[test]
fn test_select_template_default_when_no_flags() {
    let env = HashMap::new();
    assert_eq!(select_template(&env), "nginx.conf.tera");
}

#[test]
fn test_select_template_https_only() {
    let mut env = HashMap::new();
    env.insert("RIKU_HTTPS_ONLY".to_string(), "true".to_string());
    assert_eq!(select_template(&env), "nginx_https_only.conf.tera");
}

#[test]
fn test_select_template_wsgi() {
    let mut env = HashMap::new();
    env.insert("RIKU_NGINX_WSGI".to_string(), "true".to_string());
    assert_eq!(select_template(&env), "nginx_wsgi.conf.tera");
}

#[test]
fn test_select_template_portmap() {
    let mut env = HashMap::new();
    env.insert("RIKU_NGINX_PORTMAP".to_string(), "true".to_string());
    assert_eq!(select_template(&env), "nginx_portmap.conf.tera");
}

#[test]
fn test_select_template_static() {
    let mut env = HashMap::new();
    env.insert("RIKU_NGINX_STATIC".to_string(), "true".to_string());
    assert_eq!(select_template(&env), "nginx_static.conf.tera");
}

#[test]
fn test_select_template_https_only_takes_precedence_over_wsgi() {
    let mut env = HashMap::new();
    env.insert("RIKU_HTTPS_ONLY".to_string(), "true".to_string());
    env.insert("RIKU_NGINX_WSGI".to_string(), "true".to_string());
    // HTTPS_ONLY is checked first
    assert_eq!(select_template(&env), "nginx_https_only.conf.tera");
}

// register_templates

#[test]
fn test_register_templates_succeeds() {
    let mut tera = tera::Tera::default();
    let result = register_templates(&mut tera);
    assert!(result.is_ok());
}

#[test]
fn test_all_expected_templates_registered() {
    let mut tera = tera::Tera::default();
    register_templates(&mut tera).unwrap();
    let names: Vec<String> = tera.get_template_names().map(str::to_string).collect();
    assert!(names.contains(&"nginx.conf.tera".to_string()));
    assert!(names.contains(&"nginx_https_only.conf.tera".to_string()));
    assert!(names.contains(&"nginx_portmap.conf.tera".to_string()));
    assert!(names.contains(&"nginx_static.conf.tera".to_string()));
    assert!(names.contains(&"nginx_wsgi.conf.tera".to_string()));
}

#[test]
fn https_only_false_keeps_plain_template() {
    let env = HashMap::from([("RIKU_HTTPS_ONLY".to_string(), "false".to_string())]);
    assert_eq!(select_template(&env), "nginx.conf.tera");
}
