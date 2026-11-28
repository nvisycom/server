//! Workspace resolution extractor.
//!
//! Resolves the `{workspaceId}` path segment to the addressed [`Workspace`], so
//! handlers receive a validated workspace without repeating the lookup. The `id`
//! is the workspace's public URL identity and the internal key used for
//! authorization and scoped queries; the handle is a display-only name.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{Operation, Response};
use axum::RequestPartsExt;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use nvisy_postgres::model::Workspace;
use nvisy_postgres::query::WorkspaceRepository;
use nvisy_postgres::{PgClient, PgConn};
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::Span;
use uuid::Uuid;

use crate::extract::Path;
use crate::response::{Error, Resource, error_responses};

/// The workspace addressed by the `{workspaceId}` path segment.
///
/// Extracting this resolves the id to a live, non-deleted [`Workspace`]. An id
/// that matches no workspace rejects with `404 Not Found` — the same response
/// any unknown resource id produces.
///
/// The resolved [`Workspace::id`] is the value handlers pass to
/// `authorize_workspace` and the workspace-scoped repository methods.
#[must_use]
#[derive(Debug, Clone)]
pub struct WorkspaceContext(pub Workspace);

/// The `{workspaceId}` path segment. Named to match the `OpenAPI` parameter and the
/// route definition.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct IdParam {
    /// Workspace identifier.
    workspace_id: Uuid,
}

impl<S> FromRequestParts<S> for WorkspaceContext
where
    PgClient: FromRef<S>,
    S: Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let workspace_id = workspace_path_id(parts).await?;
        let mut conn = PgClient::from_ref(state).get_connection().await?;
        Ok(WorkspaceContext(
            find_workspace(&mut conn, workspace_id).await?,
        ))
    }
}

/// Reads the `{workspaceId}` path segment.
pub(crate) async fn workspace_path_id(parts: &mut Parts) -> Result<Uuid, Error> {
    let Path(IdParam { workspace_id }) = parts.extract::<Path<IdParam>>().await?;
    Ok(workspace_id)
}

/// Loads the addressed workspace, a 404 if it does not exist, and records its
/// id on the request span so every log line of the request carries it.
pub(crate) async fn find_workspace(
    conn: &mut PgConn,
    workspace_id: Uuid,
) -> Result<Workspace, Error> {
    let workspace = conn
        .find_workspace_by_id(workspace_id)
        .await?
        .ok_or_else(|| Error::not_found(Resource::Workspace))?;
    Span::current().record("workspace_id", tracing::field::display(workspace.id));
    Ok(workspace)
}

impl OperationInput for WorkspaceContext {
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        Path::<IdParam>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
        error_responses(ctx, operation, &[400, 404])
    }
}
