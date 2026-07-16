//! Workspace resolution extractor.
//!
//! Resolves the `{workspaceSlug}` path segment to the addressed [`Workspace`],
//! so handlers receive a validated workspace (and its `id`) without repeating
//! the lookup. The slug is the workspace's public URL identity; its `id`
//! remains the internal key used for authorization and scoped queries.

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

use crate::extract::Path;
use crate::handler::{Error, ErrorKind};

/// The workspace addressed by the `{workspaceSlug}` path segment.
///
/// Extracting this resolves the slug to a live, non-deleted [`Workspace`]. A
/// slug that matches no workspace rejects with `404 Not Found` — the same
/// response an unknown resource id produces, so the slug cannot be used to
/// probe which workspaces exist beyond what the caller can already reach.
///
/// The resolved [`Workspace::id`] is the value handlers pass to
/// `authorize_workspace` and the workspace-scoped repository methods.
#[must_use]
#[derive(Debug, Clone)]
pub struct WorkspaceContext(pub Workspace);

impl WorkspaceContext {
    /// Returns the resolved workspace.
    #[inline]
    #[must_use]
    pub fn workspace(&self) -> &Workspace {
        &self.0
    }

    /// Returns the resolved workspace's identifier.
    #[inline]
    #[must_use]
    pub fn id(&self) -> uuid::Uuid {
        self.0.id
    }

    /// Consumes the context, returning the owned workspace.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> Workspace {
        self.0
    }
}

/// The `{workspaceSlug}` path segment. Named to match the OpenAPI parameter and
/// the route definition.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSlugParam {
    /// URL-safe workspace identifier.
    workspace_slug: String,
}

impl<S> FromRequestParts<S> for WorkspaceContext
where
    PgClient: FromRef<S>,
    S: Sync,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(WorkspaceSlugParam { workspace_slug }) =
            parts.extract::<Path<WorkspaceSlugParam>>().await?;

        let pg_client = PgClient::from_ref(state);
        let mut conn = pg_client.get_connection().await.map_err(|error| {
            tracing::error!(error = %error, "Failed to acquire database connection");
            ErrorKind::InternalServerError
                .with_message("Database connection unavailable")
                .with_context(error.to_string())
        })?;

        let workspace = conn
            .find_workspace_by_slug(&workspace_slug)
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
        Path::<WorkspaceSlugParam>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
        Path::<WorkspaceSlugParam>::inferred_early_responses(ctx, operation)
    }
}
