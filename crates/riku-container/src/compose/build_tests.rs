use crate::compose::test_support::{service_error as error_of, ServiceFixture as Fixture};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn build_context_may_not_pass_through_a_symlink() {
    let fx = Fixture::new();
    let outside = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), fx.app.path().join("src")).unwrap();
    std::fs::create_dir(fx.app.path().join("real")).unwrap();

    for build in [
        json!("./src"),
        json!({"context": "./src/api"}),
        json!({"context": "./real", "dockerfile": "../src/Dockerfile"}),
    ] {
        let err = error_of(fx.parse("web", json!({ "build": build })));
        assert!(
            err.contains("service 'web'") && err.contains("symlink") && err.contains("src"),
            "{build}: {err}"
        );
    }
    assert!(fx
        .parse("web", json!({"build": "./real/not-created-yet"}))
        .is_ok());
}

#[test]
fn image_or_build_is_required_and_build_stays_inside_the_app() {
    let fx = Fixture::new();
    assert!(error_of(fx.parse("web", json!({}))).contains("image or build"));
    assert!(error_of(fx.parse("web", json!({"image": "-rm"}))).contains("-rm"));

    let svc = fx
        .parse(
            "web",
            json!({"build": {"context": "./api", "dockerfile": "Dockerfile.prod", "args": {"N": 2}}}),
        )
        .unwrap();
    let build = svc.build.unwrap();
    assert_eq!(build.context, fx.app.path().join("api"));
    assert_eq!(
        build.dockerfile,
        Some(fx.app.path().join("api/Dockerfile.prod"))
    );
    assert_eq!(build.args["N"], "2");

    for build in [
        json!("../elsewhere"),
        json!({"context": ".", "dockerfile": "../../x"}),
        json!({"context": ".", "target": "prod"}),
    ] {
        let err = error_of(fx.parse("web", json!({ "build": build })));
        assert!(err.contains("service 'web'"), "{err}");
    }
}

#[test]
fn dockerfile_outside_the_build_context_is_refused() {
    // Inside the app but beside the context: the engine refuses it at build
    // time, so the compose file is refused at parse time instead.
    let err = error_of(Fixture::new().parse(
        "web",
        json!({"build": {"context": "./api", "dockerfile": "../Dockerfile"}}),
    ));

    assert!(
        err.contains("service 'web'") && err.contains("build context"),
        "{err}"
    );
}
