//! Checks on the generated `OpenAPI` document for the full built-in router.
//!
//! Requires the `test_util` feature (which brings up the container helpers) and
//! Docker; run with `cargo test -p nvisy-server --features test_util`.

#![cfg(feature = "test_util")]

use nvisy_server::test_util::TestApp;
use serde_json::Value;

/// Every operation's security requirement names a scheme the document declares,
/// so a generated client knows how to send the credential.
#[tokio::test]
async fn every_security_requirement_names_a_declared_scheme() {
    let app = TestApp::spawn().await;
    let spec = app.openapi().await;

    let schemes = &spec["components"]["securitySchemes"];
    assert_eq!(schemes["BearerAuth"]["type"], "http");
    assert_eq!(schemes["BearerAuth"]["scheme"], "bearer");

    let paths = spec["paths"].as_object().expect("the spec has paths");
    let mut secured = 0;
    for (path, item) in paths {
        for (method, operation) in item.as_object().expect("a path item is an object") {
            let Some(requirements) = operation.get("security").and_then(Value::as_array) else {
                continue;
            };
            for requirement in requirements {
                for name in requirement
                    .as_object()
                    .expect("a requirement is an object")
                    .keys()
                {
                    assert!(
                        schemes.get(name).is_some(),
                        "{method} {path} requires undeclared scheme `{name}`",
                    );
                    secured += 1;
                }
            }
        }
    }
    assert!(
        secured > 0,
        "authenticated operations declare a requirement"
    );
}

/// The detection create operation declares the optional `Idempotency-Key`
/// header it accepts, with the length bound the server enforces.
#[tokio::test]
async fn detection_create_declares_the_idempotency_key() {
    let app = TestApp::spawn().await;
    let spec = app.openapi().await;

    let parameters = spec["paths"]["/workspaces/{workspaceId}/detections"]["post"]["parameters"]
        .as_array()
        .expect("the operation has parameters");
    let header = parameters
        .iter()
        .find(|p| p["in"] == "header" && p["name"] == "Idempotency-Key")
        .expect("the Idempotency-Key header is declared");
    // `required: false` is the default, so the document may omit it.
    assert!(
        header
            .get("required")
            .is_none_or(|required| required == false)
    );
    assert_eq!(header["schema"]["minLength"], 1);
    assert_eq!(header["schema"]["maxLength"], 255);
}
