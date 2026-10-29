//! HTTP contract and authorization for the workspace detection-tree listing.
#![cfg(feature = "test_util")]

use nvisy_server::handler::response::DetectionTreesPage;
use nvisy_server::test_util::TestApp;

#[tokio::test]
async fn detection_trees_is_a_workspace_scoped_page() {
    let app = TestApp::spawn().await;
    let (owner, workspace) = app.workspace_owner().await;
    let path = format!("/workspaces/{workspace}/detections/trees?limit=1&includeCount=true");
    let response = app
        .server()
        .get(&path)
        .authorization_bearer(&owner.jwt)
        .await;
    response.assert_status_ok();
    let page: DetectionTreesPage = response.json();
    assert_eq!(page.items.len(), 0);
    assert_eq!(page.total, Some(0));
    assert!(page.next_cursor.is_none());

    let outsider = app.actor().await;
    app.server()
        .get(&path)
        .authorization_bearer(&outsider.jwt)
        .await
        .assert_status_forbidden();
    app.server().get(&path).await.assert_status_unauthorized();
}
