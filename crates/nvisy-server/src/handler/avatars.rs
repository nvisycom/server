//! Public avatar serving.
//!
//! Avatars are served from a dedicated, unauthenticated route family keyed by
//! the owner's opaque id, so they load directly in an `<img>` tag and reveal no
//! username or slug. Each URL carries the `version` segment an upload drew, so a
//! URL maps to one stored object and the response is marked immutable. Uploads
//! and deletes live with the authenticated account and workspace handlers.

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;

use crate::extract::{Json, Path};
use crate::handler::request::AvatarPathParams;
use crate::response::{AvatarImage, Error, ErrorResponse, Resource, Result};
use crate::service::{AvatarOwner, AvatarService, ServiceState};

/// Tracing target for public avatar serving.
const TRACING_TARGET: &str = "nvisy_server::handler::avatars";

/// Serves an account's avatar image. Public; 404 when the version is unknown.
#[tracing::instrument(skip_all)]
async fn get_account_avatar(
    State(avatar): State<AvatarService>,
    Path(params): Path<AvatarPathParams>,
) -> Result<AvatarImage> {
    tracing::debug!(target: TRACING_TARGET, "Serving account avatar");

    let bytes = avatar
        .get(AvatarOwner::Account(params.id), &params.version)
        .await?
        .ok_or_else(|| Error::not_found(Resource::Avatar))?;

    Ok(AvatarImage(bytes))
}

/// Serves a workspace's avatar image. Public; 404 when the version is unknown.
#[tracing::instrument(skip_all)]
async fn get_workspace_avatar(
    State(avatar): State<AvatarService>,
    Path(params): Path<AvatarPathParams>,
) -> Result<AvatarImage> {
    tracing::debug!(target: TRACING_TARGET, "Serving workspace avatar");

    let bytes = avatar
        .get(AvatarOwner::Workspace(params.id), &params.version)
        .await?
        .ok_or_else(|| Error::not_found(Resource::Avatar))?;

    Ok(AvatarImage(bytes))
}

fn get_avatar_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get avatar")
        .description("Returns the owner's avatar image (WebP), publicly. 404 when unset.")
        .response::<404, Json<ErrorResponse>>()
}

/// Returns the public avatar-serving routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/avatars/accounts/{id}/{version}",
            get_with(get_account_avatar, get_avatar_docs),
        )
        .api_route(
            "/avatars/workspaces/{id}/{version}",
            get_with(get_workspace_avatar, get_avatar_docs),
        )
        .with_path_items(|item| item.tag("Avatars"))
}
