//! Public avatar serving.
//!
//! Avatars are served from a dedicated, unauthenticated route family keyed by
//! the owner's opaque id, so they load directly in an `<img>` tag and reveal no
//! username or slug. Each URL carries a content-hash `version` segment used only
//! for cache-busting; the route serves the owner's current avatar regardless of
//! its value and marks the response immutable. Uploads and deletes live with the
//! authenticated account and workspace handlers.

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::response::Response;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::{Json, Path};
use crate::handler::response::ErrorResponse;
use crate::handler::utility::avatar_response;
use crate::handler::{Error, Result};
use crate::service::{AvatarService, ServiceState};

/// Tracing target for public avatar serving.
const TRACING_TARGET: &str = "nvisy_server::handler::avatars";

/// Path parameters for a public avatar route: the owner id and a cache-busting
/// content-hash segment (ignored when serving).
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct AvatarPathParams {
    /// Opaque id of the avatar's owner.
    id: Uuid,
    /// Content-hash cache-busting segment (ignored when serving).
    version: String,
}

/// Serves an account's avatar image. Public; 404 when unset.
#[tracing::instrument(skip_all)]
async fn get_account_avatar(
    State(avatar): State<AvatarService>,
    Path(params): Path<AvatarPathParams>,
) -> Result<Response> {
    tracing::debug!(target: TRACING_TARGET, "Serving account avatar");

    let bytes = avatar
        .account_avatar(params.id)
        .await?
        .ok_or_else(|| Error::not_found("avatar"))?;

    Ok(avatar_response(bytes))
}

/// Serves a workspace's avatar image. Public; 404 when unset.
#[tracing::instrument(skip_all)]
async fn get_workspace_avatar(
    State(avatar): State<AvatarService>,
    Path(params): Path<AvatarPathParams>,
) -> Result<Response> {
    tracing::debug!(target: TRACING_TARGET, "Serving workspace avatar");

    let bytes = avatar
        .workspace_avatar(params.id)
        .await?
        .ok_or_else(|| Error::not_found("avatar"))?;

    Ok(avatar_response(bytes))
}

fn get_avatar_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get avatar")
        .description("Returns the owner's avatar image (WebP), publicly. 404 when unset.")
        .response::<200, ()>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns the public avatar-serving routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/avatars/accounts/{id}/{version}/",
            get_with(get_account_avatar, get_avatar_docs),
        )
        .api_route(
            "/avatars/workspaces/{id}/{version}/",
            get_with(get_workspace_avatar, get_avatar_docs),
        )
        .with_path_items(|item| item.tag("Avatars"))
}
