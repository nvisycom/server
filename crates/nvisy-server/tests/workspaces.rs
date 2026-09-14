//! End-to-end tests for the workspace handlers, run against a real containerized
//! server via [`TestApp`].
//!
//! Requires the `test_util` feature (which brings up the container helpers) and
//! Docker; run with `cargo test -p nvisy-server --features test_util`.

#![cfg(feature = "test_util")]

use nvisy_postgres::types::WorkspaceRole;
use nvisy_server::test_util::TestApp;

/// A state-changing request (DELETE) as an owner succeeds — and does so with a
/// bearer token and no CSRF header, proving bearer transport is CSRF-exempt.
#[tokio::test]
async fn delete_workspace_as_owner_succeeds() {
    let app = TestApp::spawn().await;
    let (owner, workspace) = app.workspace_owner().await;

    let response = app
        .server()
        .delete(&format!("/workspaces/{workspace}"))
        .authorization_bearer(&owner.jwt)
        .await;

    // Delete requires the Owner role; the owner is authorized.
    assert!(
        response.status_code().is_success(),
        "owner delete should succeed, got {}",
        response.status_code(),
    );
}

/// The same DELETE as a lower-role member (Reviewer) is forbidden (403): the
/// role gate on `DeleteWorkspace` (Owner-only) is enforced.
#[tokio::test]
async fn delete_workspace_as_reviewer_is_forbidden() {
    let app = TestApp::spawn().await;
    let (member, workspace) = app.workspace_member(WorkspaceRole::Reviewer).await;

    let response = app
        .server()
        .delete(&format!("/workspaces/{workspace}"))
        .authorization_bearer(&member.jwt)
        .await;

    response.assert_status_forbidden();
}
