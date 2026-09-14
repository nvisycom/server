//! End-to-end tests for the workspace-members handlers, run against a real
//! containerized server via [`TestApp`].
//!
//! Requires the `test_util` feature (which brings up the container helpers) and
//! Docker; run with `cargo test -p nvisy-server --features test_util`.

#![cfg(feature = "test_util")]

use nvisy_server::handler::response::WorkspaceMembersPage;
use nvisy_server::test_util::TestApp;
use uuid::Uuid;

/// A member can list a workspace's members, and the response carries the
/// member's account id (needed to address them, e.g. as a review assignee).
#[tokio::test]
async fn list_members_as_owner_returns_the_membership() {
    let app = TestApp::spawn().await;
    let (owner, workspace) = app.workspace_owner().await;

    let response = app
        .server()
        .get(&format!("/workspaces/{workspace}/members"))
        .authorization_bearer(&owner.jwt)
        .await;

    response.assert_status_ok();
    let page: WorkspaceMembersPage = response.json();
    assert!(
        page.items.iter().any(|m| m.id == owner.account_id),
        "the owner should appear in the member list, addressable by id",
    );
}

/// An authenticated account that is not a member of the workspace is forbidden
/// (403), exercising the role/membership check after the token is accepted.
#[tokio::test]
async fn list_members_as_non_member_is_forbidden() {
    let app = TestApp::spawn().await;
    let (_owner, workspace) = app.workspace_owner().await;
    // A second, valid actor with no membership in `workspace`.
    let outsider = app.actor().await;

    let response = app
        .server()
        .get(&format!("/workspaces/{workspace}/members"))
        .authorization_bearer(&outsider.jwt)
        .await;

    response.assert_status_forbidden();
}

/// An unknown workspace id resolves to 404, not 403 — the `WorkspaceContext`
/// extractor rejects before the authorization check.
#[tokio::test]
async fn list_members_for_unknown_workspace_is_not_found() {
    let app = TestApp::spawn().await;
    let actor = app.actor().await;
    let missing = Uuid::now_v7();

    let response = app
        .server()
        .get(&format!("/workspaces/{missing}/members"))
        .authorization_bearer(&actor.jwt)
        .await;

    response.assert_status_not_found();
}

/// No bearer token at all is rejected as unauthenticated (401), proving the
/// route is behind the auth layer.
#[tokio::test]
async fn list_members_without_a_token_is_unauthorized() {
    let app = TestApp::spawn().await;
    let (_owner, workspace) = app.workspace_owner().await;

    let response = app
        .server()
        .get(&format!("/workspaces/{workspace}/members"))
        .await;

    response.assert_status_unauthorized();
}
