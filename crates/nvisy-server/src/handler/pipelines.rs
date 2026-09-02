//! Pipeline management handlers for CRUD operations.
//!
//! This module provides comprehensive pipeline management functionality including
//! creating, reading, updating, deleting pipelines, and listing pipelines within
//! a workspace. All operations are secured with role-based access control.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::WorkspacePipeline;
use nvisy_postgres::query::{
    PipelineReferenceRepository, WorkspaceFileRepository, WorkspacePipelineRepository,
};
use nvisy_postgres::types::{FileKind, Handle, RetentionScope, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn, PgConnection, PgResult};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, SecurityContext, ValidateJson,
    WorkspaceContext,
};
use crate::handler::request::{
    CreatePipeline, CursorPagination, PipelineFilter, PipelinePathParams, PipelineReferences,
    UpdatePipeline,
};
use crate::handler::response::{AccountRef, ErrorResponse, Page, Pipeline, PipelineSummary};
use crate::handler::utility::resolve_account_ref;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{EventEmitter, EventOrigin, PipelineRef, ServiceState, WorkspaceEvent};

/// Tracing target for pipeline operations.
const TRACING_TARGET: &str = "nvisy_server::handler::pipelines";

/// Creates a new pipeline within a workspace.
///
/// The creator is recorded as the pipeline's owner. Requires the
/// `CreatePipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn create_pipeline(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreatePipeline>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating pipeline");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::CreatePipelines)
        .await?;

    let (new_pipeline, references) = request
        .into_parts(workspace.id, auth_state.account_id)
        .map_err(serialize_error)?;

    let policy_ids = resolve_references(&mut conn, workspace.id, &references).await?;

    let pipeline = conn
        .transaction(async |conn| {
            let pipeline = conn.create_workspace_pipeline(new_pipeline).await?;
            replace_references(conn, &pipeline, &policy_ids).await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id: auth_state.account_id,
                    security: &security,
                },
                WorkspaceEvent::PipelineCreated(PipelineRef {
                    pipeline_id: pipeline.id,
                    pipeline_slug: pipeline.slug.clone(),
                }),
            )
            .await?;
            Ok::<WorkspacePipeline, Error>(pipeline)
        })
        .await?;

    // The creator is the authenticated caller; resolve their handle directly.
    let creator = resolve_account_ref(&mut conn, auth_state.account_id).await?;

    // The references were just written from the request, so build the response
    // from its slugs directly instead of reading the join table back.
    let response = Pipeline::from_model(pipeline, workspace.slug, creator, references.policy_slugs)
        .map_err(serialize_error)?;

    tracing::info!(
        target: TRACING_TARGET,
        pipeline_slug = %response.slug,
        "Pipeline created",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn create_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create pipeline")
        .description("Creates a new pipeline in the workspace. The creator is set as the owner.")
        .response::<201, Json<Pipeline>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all pipelines in a workspace with optional filtering.
///
/// Supports filtering by status and searching by name. Requires the
/// `ViewPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_pipelines(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
    Query(filter): Query<PipelineFilter>,
) -> Result<(StatusCode, Json<Page<PipelineSummary>>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipelines");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let page = conn
        .cursor_list_workspace_pipelines(
            workspace.id,
            pagination.into(),
            filter.status,
            filter.search.as_deref(),
        )
        .await?;

    let response = Page::from_cursor_page(page, |wc| {
        PipelineSummary::from_model(wc.item, workspace.slug.clone(), wc.account.into())
    });

    tracing::debug!(
        target: TRACING_TARGET,
        pipeline_count = response.items.len(),
        "Pipelines listed",
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_pipelines_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipelines")
        .description("Returns all pipelines in the workspace with optional filtering by status and name search.")
        .response::<200, Json<Page<PipelineSummary>>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a pipeline by slug.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        pipeline_slug = %path_params.pipeline_slug,
    )
)]
async fn get_pipeline(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelinePathParams>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting pipeline");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let found = find_pipeline(&mut conn, workspace.id, &path_params.pipeline_slug).await?;
    let pipeline = found.item;
    let creator: AccountRef = found.account.into();

    let policy_slugs = conn.list_pipeline_policy_slugs(pipeline.id).await?;

    let response = Pipeline::from_model(pipeline, workspace.slug, creator, policy_slugs)
        .map_err(serialize_error)?;

    tracing::info!(target: TRACING_TARGET, "Pipeline retrieved");

    Ok((StatusCode::OK, Json(response)))
}

fn get_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get pipeline")
        .description("Returns a pipeline by its slug.")
        .response::<200, Json<Pipeline>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates an existing pipeline.
///
/// Only provided fields are updated. Requires the `UpdatePipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        pipeline_slug = %path_params.pipeline_slug,
    )
)]
async fn update_pipeline(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelinePathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdatePipeline>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating pipeline");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::UpdatePipelines)
        .await?;

    // Confirm the pipeline exists in this workspace before mutating.
    let found = find_pipeline(&mut conn, workspace.id, &path_params.pipeline_slug).await?;
    let existing = found.item;
    let creator: AccountRef = found.account.into();

    let (update_data, references, retention_override) =
        request.into_parts().map_err(serialize_error)?;
    let pipeline_id = existing.id;

    // When a definition is supplied, resolve its slugs to ids up front so an
    // unknown reference rejects with 404 before any write.
    let resolved = match &references {
        Some(references) => Some(resolve_references(&mut conn, workspace.id, references).await?),
        None => None,
    };

    // If the request changed the retention override, recompute `expires_at` on
    // the files this pipeline already produced, so the change applies to stored
    // data too. Resolved against the workspace baseline (override wins per scope).
    let backfill = retention_override.map(|over| {
        let workspace_retention = workspace.settings.or_default().retention;
        let now = jiff::Timestamp::now();
        let audit_logs_expiry = workspace_retention
            .resolve(RetentionScope::AuditLogs, Some(&over))
            .expires_at(now);
        [
            (
                FileKind::Redacted,
                workspace_retention
                    .resolve(RetentionScope::RedactedDocuments, Some(&over))
                    .expires_at(now),
            ),
            (FileKind::Audit, audit_logs_expiry),
            // Review audits share the audit-logs scope with detection audits.
            (FileKind::Review, audit_logs_expiry),
            (
                FileKind::Intermediate,
                workspace_retention
                    .resolve(RetentionScope::Intermediates, Some(&over))
                    .expires_at(now),
            ),
        ]
    });

    let pipeline = conn
        .transaction(async |conn| {
            let pipeline = conn
                .update_workspace_pipeline(pipeline_id, update_data)
                .await?;
            // Only touch the join table when the request supplied a definition.
            if let Some(policy_ids) = &resolved {
                replace_references(conn, &pipeline, policy_ids).await?;
            }
            // Backfill produced files' expiry when the override changed.
            if let Some(pairs) = &backfill {
                for (kind, expires_at) in pairs {
                    conn.backfill_pipeline_files_expiry(pipeline_id, *kind, *expires_at)
                        .await?;
                }
            }
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id: auth_state.account_id,
                    security: &security,
                },
                WorkspaceEvent::PipelineUpdated(PipelineRef {
                    pipeline_id,
                    pipeline_slug: pipeline.slug.clone(),
                }),
            )
            .await?;
            Ok::<WorkspacePipeline, Error>(pipeline)
        })
        .await?;

    let response = match references {
        // A definition was supplied: the references we just wrote are current.
        Some(references) => {
            Pipeline::from_model(pipeline, workspace.slug, creator, references.policy_slugs)
                .map_err(serialize_error)?
        }
        // Partial update left the references untouched: read them back.
        None => build_response(&mut conn, pipeline, workspace.slug, creator).await?,
    };

    tracing::info!(target: TRACING_TARGET, "Pipeline updated");

    Ok((StatusCode::OK, Json(response)))
}

fn update_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update pipeline")
        .description("Updates an existing pipeline. Only provided fields are updated.")
        .response::<200, Json<Pipeline>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Soft-deletes a pipeline.
///
/// Requires the `DeletePipelines` permission. The pipeline is marked as deleted
/// but data is retained for potential recovery.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        pipeline_slug = %path_params.pipeline_slug,
    )
)]
async fn delete_pipeline(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelinePathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting pipeline");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::DeletePipelines)
        .await?;

    // Confirm the pipeline exists in this workspace before deleting.
    let existing = find_pipeline(&mut conn, workspace.id, &path_params.pipeline_slug)
        .await?
        .item;
    let pipeline_id = existing.id;
    let pipeline_slug = existing.slug.clone();

    // Delete the pipeline and record the event in one transaction so the two
    // commit atomically.
    conn.transaction(async |conn| {
        conn.delete_workspace_pipeline(pipeline_id).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: auth_state.account_id,
                security: &security,
            },
            WorkspaceEvent::PipelineDeleted(PipelineRef {
                pipeline_id,
                pipeline_slug,
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    tracing::info!(target: TRACING_TARGET, "Pipeline deleted");

    Ok(StatusCode::OK)
}

fn delete_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete pipeline")
        .description("Soft-deletes a pipeline. Data is retained for potential recovery.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a pipeline within a workspace by slug, with its creator, or returns a
/// NotFound error.
async fn find_pipeline(
    conn: &mut PgConn,
    workspace_id: Uuid,
    pipeline_slug: &str,
) -> Result<WithAccountRef<WorkspacePipeline>> {
    conn.find_pipeline_in_workspace_by_slug(workspace_id, pipeline_slug)
        .await?
        .ok_or_else(|| Error::not_found("pipeline"))
}

/// Replaces a pipeline's policy references in the join table.
///
/// Run inside the same transaction as the pipeline write so the config JSON and
/// its references stay consistent.
async fn replace_references(
    conn: &mut PgConnection,
    pipeline: &WorkspacePipeline,
    policy_ids: &[Uuid],
) -> PgResult<()> {
    conn.replace_workspace_pipeline_policies(pipeline.workspace_id, pipeline.id, policy_ids)
        .await?;
    Ok(())
}

/// Resolves a set of policy slugs to their ids within a workspace, rejecting the
/// whole request with a 404 if any slug is unknown.
async fn resolve_references(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    references: &PipelineReferences,
) -> Result<Vec<Uuid>> {
    let policy_ids = conn
        .resolve_policy_slugs(workspace_id, &references.policy_slugs)
        .await?
        .ok_or_else(|| Error::not_found("policy"))?;
    Ok(policy_ids)
}

/// Builds a [`Pipeline`] response, reading the pipeline's (live) references back
/// from the join table. Used when the caller did not just write them.
async fn build_response(
    conn: &mut PgConnection,
    pipeline: WorkspacePipeline,
    workspace_slug: Handle,
    creator: AccountRef,
) -> Result<Pipeline> {
    let policy_slugs = conn.list_pipeline_policy_slugs(pipeline.id).await?;
    Pipeline::from_model(pipeline, workspace_slug, creator, policy_slugs).map_err(serialize_error)
}

/// Maps a definition (de)serialization failure to an internal error.
///
/// A stored definition that will not round-trip is a server-side data problem,
/// not a client error.
fn serialize_error(error: serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to process pipeline definition")
        .with_context(error.to_string())
}

/// Returns a [`Router`] with all pipeline-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        // Workspace-scoped routes for listing and creating
        .api_route(
            "/workspaces/{workspaceSlug}/pipelines/",
            post_with(create_pipeline, create_pipeline_docs)
                .get_with(list_pipelines, list_pipelines_docs),
        )
        // Pipeline operations by slug
        .api_route(
            "/workspaces/{workspaceSlug}/pipelines/{pipelineSlug}/",
            get_with(get_pipeline, get_pipeline_docs)
                .patch_with(update_pipeline, update_pipeline_docs)
                .delete_with(delete_pipeline, delete_pipeline_docs),
        )
        .with_path_items(|item| item.tag("Pipelines"))
}
