//! Pipeline management handlers for CRUD operations.
//!
//! These handlers are thin: they authorize, parse the request, delegate the
//! pipeline rules to [`WorkspacePipelineService`], and map the result to a response. The
//! domain logic lives in that service.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspacePipeline, CursorPagination, UpdateWorkspacePipeline, WorkspacePipelineFilter,
    WorkspacePipelinePathParams,
};
use crate::handler::response::{Page, WorkspacePipeline, WorkspacePipelineSummary};
use crate::handler::utility::resolve_account_ref;
use crate::response::{Error, ErrorResponse, Result};
use crate::service::{ServiceState, event};

/// Tracing target for pipeline operations.
const TRACING_TARGET: &str = "nvisy_server::handler::pipelines";

/// Maps a definition serialization failure to an internal error.
fn serialize_error(error: &serde_json::Error) -> Error<'static> {
    crate::response::ErrorKind::InternalServerError
        .with_message("Failed to process pipeline definition")
        .with_context(error.to_string())
}

/// Creates a new pipeline within a workspace.
///
/// The creator is recorded as the pipeline's owner. Requires the
/// `CreatePipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn create_pipeline(
    State(pg_client): State<PgClient>,
    State(pipelines): State<domain::WorkspacePipelineService>,
    authz: Authorized<markers::CreatePipelines>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspacePipeline>,
) -> Result<(StatusCode, Json<WorkspacePipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating pipeline");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    let origin = event::EventOrigin {
        workspace_id: workspace.id,
        account_id,
        security: &security,
    };

    let domain::output::PipelineWithReferences {
        pipeline,
        policy_ids,
    } = pipelines.create(origin, request.into()).await?;

    let mut conn = pg_client.get_connection().await?;
    let creator = resolve_account_ref(&mut conn, account_id).await?;

    let response = WorkspacePipeline::from_model(
        pipeline,
        workspace.id,
        workspace.handle,
        creator,
        policy_ids,
    )
    .map_err(|e| serialize_error(&e))?;

    Ok((StatusCode::CREATED, Json(response)))
}

fn create_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create pipeline")
        .description("Creates a new pipeline in the workspace. The creator is set as the owner.")
        .response::<201, Json<WorkspacePipeline>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_pipelines(
    State(pipelines): State<domain::WorkspacePipelineService>,
    authz: Authorized<markers::ViewPipelines>,
    Query(pagination): Query<CursorPagination>,
    Query(filter): Query<WorkspacePipelineFilter>,
) -> Result<(StatusCode, Json<Page<WorkspacePipelineSummary>>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipelines");

    let workspace = authz.workspace;

    let page = pipelines
        .list(
            workspace.id,
            pagination.into_cursor(),
            filter.status,
            filter.search.as_deref(),
        )
        .await?;

    let response = Page::from_cursor_page(page, |wc| {
        WorkspacePipelineSummary::from_model(
            wc.item,
            workspace.id,
            workspace.handle.clone(),
            wc.account.into(),
        )
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_pipelines_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipelines")
        .description("Returns all pipelines in the workspace with optional filtering by status and name search.")
        .response::<200, Json<Page<WorkspacePipelineSummary>>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a pipeline by id.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn get_pipeline(
    State(pipelines): State<domain::WorkspacePipelineService>,
    authz: Authorized<markers::ViewPipelines>,
    Path(path_params): Path<WorkspacePipelinePathParams>,
) -> Result<(StatusCode, Json<WorkspacePipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting pipeline");

    let workspace = authz.workspace;

    let (found, policy_ids) = pipelines
        .find(workspace.id, path_params.pipeline_id)
        .await?;

    let response = WorkspacePipeline::from_model(
        found.item,
        workspace.id,
        workspace.handle,
        found.account.into(),
        policy_ids,
    )
    .map_err(|e| serialize_error(&e))?;

    Ok((StatusCode::OK, Json(response)))
}

fn get_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get pipeline")
        .description("Returns a pipeline by its id.")
        .response::<200, Json<WorkspacePipeline>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn update_pipeline(
    State(pg_client): State<PgClient>,
    State(pipelines): State<domain::WorkspacePipelineService>,
    authz: Authorized<markers::UpdatePipelines>,
    Path(path_params): Path<WorkspacePipelinePathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspacePipeline>,
) -> Result<(StatusCode, Json<WorkspacePipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating pipeline");

    let workspace = authz.workspace;
    let origin = event::EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };

    let domain::output::PipelineWithReferences {
        pipeline,
        policy_ids,
    } = pipelines
        .update(origin, path_params.pipeline_id, request.into())
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let creator = resolve_account_ref(&mut conn, pipeline.account_id).await?;

    let response = WorkspacePipeline::from_model(
        pipeline,
        workspace.id,
        workspace.handle,
        creator,
        policy_ids,
    )
    .map_err(|e| serialize_error(&e))?;

    Ok((StatusCode::OK, Json(response)))
}

fn update_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update pipeline")
        .description("Updates an existing pipeline. Only provided fields are updated.")
        .response::<200, Json<WorkspacePipeline>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn delete_pipeline(
    State(pipelines): State<domain::WorkspacePipelineService>,
    authz: Authorized<markers::DeletePipelines>,
    Path(path_params): Path<WorkspacePipelinePathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting pipeline");

    let workspace = authz.workspace;
    let origin = event::EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };

    pipelines.delete(origin, path_params.pipeline_id).await?;

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

/// Returns a [`Router`] with all pipeline-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        // Workspace-scoped routes for listing and creating
        .api_route(
            "/workspaces/{workspaceId}/pipelines/",
            post_with(create_pipeline, create_pipeline_docs)
                .get_with(list_pipelines, list_pipelines_docs),
        )
        // Pipeline operations by id
        .api_route(
            "/workspaces/{workspaceId}/pipelines/{pipelineId}/",
            get_with(get_pipeline, get_pipeline_docs)
                .patch_with(update_pipeline, update_pipeline_docs)
                .delete_with(delete_pipeline, delete_pipeline_docs),
        )
        .with_path_items(|item| item.tag("Pipelines"))
}
