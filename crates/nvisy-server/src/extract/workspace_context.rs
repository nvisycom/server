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
use nvisy_postgres::PgClient;
use nvisy_postgres::model::Workspace;
use nvisy_postgres::query::WorkspaceRepository;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::extract::Path;
use crate::response::{Error, ErrorKind};

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

/// The `{workspaceId}` path segment. Named to match the OpenAPI parameter and the
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
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(IdParam { workspace_id }) = parts.extract::<Path<IdParam>>().await?;

        let pg_client = PgClient::from_ref(state);
        let mut conn = pg_client.get_connection().await.map_err(|error| {
            tracing::error!(error = %error, "Failed to acquire database connection");
            ErrorKind::InternalServerError
                .with_message("Database connection unavailable")
                .with_context(error.to_string())
        })?;

        let workspace = conn
            .find_workspace_by_id(workspace_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::NotFound
                    .with_message("Workspace not found")
                    .with_resource("workspace")
            })?;

        Ok(WorkspaceContext(workspace))
    }
}

impl OperationInput for WorkspaceContext {
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        Path::<IdParam>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
        Path::<IdParam>::inferred_early_responses(ctx, operation)
    }
}
