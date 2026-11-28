//! End-to-end tests for avatar upload, serving, and removal, run against a real
//! containerized server via [`TestApp`].
//!
//! Requires the `test_util` feature (which brings up the container helpers) and
//! Docker; run with `cargo test -p nvisy-server --features test_util`.

#![cfg(feature = "test_util")]

use std::io::Cursor;

use axum_test::multipart::{MultipartForm, Part};
use image::{DynamicImage, ImageFormat, RgbaImage};
use nvisy_server::handler::response::Account;
use nvisy_server::test_util::{Actor, TestApp};

/// A small PNG to upload.
fn png() -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::new(16, 16))
        .write_to(&mut out, ImageFormat::Png)
        .expect("encode png");
    out.into_inner()
}

/// Uploads `image` as the actor's avatar, returning its new serve URL.
async fn upload(app: &TestApp, actor: &Actor, image: Vec<u8>) -> String {
    let form = MultipartForm::new().add_part(
        "file",
        Part::bytes(image)
            .file_name("avatar.png")
            .mime_type("image/png"),
    );
    let response = app
        .server()
        .put(&format!("/accounts/{}/avatar", actor.account_id))
        .authorization_bearer(&actor.jwt)
        .multipart(form)
        .await;
    response.assert_status_ok();
    response
        .json::<Account>()
        .avatar_url
        .expect("the account has an avatar")
}

/// Re-uploading the same image draws a new version: the URL changes, the new
/// URL serves the image, and the replaced version is gone.
#[tokio::test]
async fn reupload_replaces_the_previous_version() {
    let app = TestApp::spawn().await;
    let actor = app.actor().await;

    let first = upload(&app, &actor, png()).await;
    let second = upload(&app, &actor, png()).await;
    assert_ne!(first, second, "each upload gets its own version");

    app.server().get(&second).await.assert_status_ok();
    app.server().get(&first).await.assert_status_not_found();
}

/// Deleting the avatar clears the URL and removes the object it named.
#[tokio::test]
async fn delete_removes_the_current_version() {
    let app = TestApp::spawn().await;
    let actor = app.actor().await;
    let url = upload(&app, &actor, png()).await;
    app.server().get(&url).await.assert_status_ok();

    app.server()
        .delete(&format!("/accounts/{}/avatar", actor.account_id))
        .authorization_bearer(&actor.jwt)
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    app.server().get(&url).await.assert_status_not_found();
}
