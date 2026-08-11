//! Pipeline run handlers: detect, review, and redact.
//!
//! A run is one analysis of a file through a pipeline. Detect creates the run
//! and stores the findings; the run then awaits reviewer verification before
//! redact consumes the verified findings and produces a redacted file.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_engine::OcrMode;
use nvisy_engine::policy::PolicyDefinition;
use nvisy_postgres::model::{
    NewWorkspacePipelineRun, UpdateWorkspacePipelineRun, WorkspacePipeline, WorkspacePipelineRun,
};
use nvisy_postgres::query::{
    PipelineReferenceRepository, WorkspaceFileRepository, WorkspacePipelineRepository,
    WorkspacePipelineRunRepository, WorkspacePolicyRepository,
};
use nvisy_postgres::types::{OcrPolicy, PipelineRunStatus, WorkspaceSettings};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, IdempotencyKey, Json, Path, Permission, Query, ValidateJson,
    WorkspaceContext,
};
use crate::handler::request::{
    CreatePipelineRun, CursorPagination, PipelineDefinition, PipelinePathParams,
    PipelineRunPathParams, WorkspaceRunsQuery,
};
use crate::handler::response::{ErrorResponse, PipelineRun, PipelineRunsPage};
use crate::handler::utility::resolve_account_ref;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{BlobService, CryptoService, EngineService, ServiceState, WebhookEmitter};

/// Tracing target for pipeline run operations.
const TRACING_TARGET: &str = "nvisy_server::handler::runs";

/// Starts a run: analyzes a file with the pipeline's configuration (detect).
///
/// Returns the run holding the findings for review. A repeated request with the
/// same `Idempotency-Key` returns the existing run instead of analyzing again.
/// Requires `RunPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        pipeline_slug = %path_params.pipeline_slug,
    )
)]
async fn create_pipeline_run(
    State(pg_client): State<PgClient>,
    State(blob): State<BlobService>,
    State(crypto): State<CryptoService>,
    State(engine): State<EngineService>,
    State(webhook_emitter): State<WebhookEmitter>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelinePathParams>,
    IdempotencyKey(idempotency_key): IdempotencyKey,
    ValidateJson(request): ValidateJson<CreatePipelineRun>,
) -> Result<(StatusCode, Json<PipelineRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting pipeline run (detect)");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::RunPipelines)
        .await?;

    let pipeline = find_pipeline(&mut conn, workspace.id, &path_params.pipeline_slug).await?;

    // Only an enabled pipeline runs: a draft (still being configured) or a
    // disabled (paused) pipeline is rejected.
    if !pipeline.status.is_enabled() {
        return Err(ErrorKind::Conflict
            .with_message("Pipeline is not enabled")
            .with_resource("pipeline"));
    }

    // Idempotent replay: a repeated key returns the run created the first time,
    // attributed to whoever originally triggered it (not the current caller).
    if let Some(key) = &idempotency_key
        && let Some(existing) = conn
            .find_pipeline_run_by_idempotency_key(pipeline.id, key)
            .await?
    {
        tracing::debug!(target: TRACING_TARGET, "Replaying run for idempotency key");
        let trigger = resolve_account_ref(&mut conn, existing.account_id).await?;
        return Ok((
            StatusCode::OK,
            Json(PipelineRun::from_model(
                existing,
                pipeline.slug.clone(),
                workspace.slug.clone(),
                trigger,
            )),
        ));
    }

    let file = conn
        .find_file_in_workspace(pipeline.workspace_id, request.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;

    let definition = PipelineDefinition::from_parts(pipeline.definition.clone(), Vec::new())
        .map_err(serialize_error)?;

    // Create the run first so its id is the engine correlation id.
    let new_run = NewWorkspacePipelineRun {
        pipeline_id: pipeline.id,
        input_file_id: file.id,
        account_id: auth_state.account_id,
        status: Some(PipelineRunStatus::Running),
        idempotency_key: idempotency_key.clone(),
        ..Default::default()
    };
    let run = conn.create_workspace_pipeline_run(new_run).await?;

    if let Err(err) = webhook_emitter
        .emit_pipeline_run_started(
            workspace.id,
            run.id,
            Some(auth_state.account_id),
            Some(serde_json::json!({ "pipelineId": pipeline.id, "fileId": file.id })),
        )
        .await
    {
        tracing::warn!(
            target: TRACING_TARGET,
            error = %err,
            run_id = %run.id,
            "Failed to emit pipeline:run.started webhook event"
        );
    }

    // Map the workspace's OCR policy to the engine's per-run mode. `Force` renders
    // every page at the engine's default DPI (no workspace DPI knob).
    let ocr_mode = match WorkspaceSettings::from_value(&workspace.settings).ocr {
        OcrPolicy::Auto => OcrMode::Auto,
        OcrPolicy::Force => OcrMode::force(),
        OcrPolicy::Never => OcrMode::Never,
    };

    // Build the analyzer params from the pipeline's intent; the deployment's
    // recognizer lineups and enrichers are engine-owned.
    let params = engine.analyzer_params(&definition, request.scope, ocr_mode);

    let document = blob.build_document(&file, run.id).await?;

    // The pipeline's policies own the label vocabulary: detection derives its
    // catalog from them, so the analysis emits exactly what the policies can act
    // on. Resolved before analyze and reused verbatim at redact.
    let policies = resolve_policies(&mut conn, &crypto, workspace.id, pipeline.id).await?;
    if policies.is_empty() {
        fail_run(
            &mut conn,
            &webhook_emitter,
            workspace.id,
            run.id,
            auth_state.account_id,
        )
        .await;
        return Err(ErrorKind::BadRequest
            .with_message("Pipeline has no policies; attach at least one before running")
            .with_resource("pipeline"));
    }

    let analyzed = match engine.analyze(document, &policies, &params).await {
        Ok(analyzed) => analyzed,
        Err(err) => {
            fail_run(
                &mut conn,
                &webhook_emitter,
                workspace.id,
                run.id,
                auth_state.account_id,
            )
            .await;
            return Err(err.into());
        }
    };

    // The analysis is a map of detected PII; encrypt it and record it as an
    // audit-kind file, keeping only its id on the run.
    let workspace_settings = WorkspaceSettings::from_value(&workspace.settings).retention;
    let audit_file_id = blob
        .store_analyzed_document(
            &mut conn,
            &pipeline,
            &workspace_settings,
            auth_state.account_id,
            &analyzed,
        )
        .await?;
    let run = conn
        .update_workspace_pipeline_run(
            run.id,
            UpdateWorkspacePipelineRun {
                status: Some(PipelineRunStatus::Analyzed),
                audit_file_id: Some(Some(audit_file_id)),
                ..Default::default()
            },
        )
        .await?;

    tracing::info!(target: TRACING_TARGET, run_id = %run.id, "Pipeline run analyzed");

    let trigger = resolve_account_ref(&mut conn, run.account_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(PipelineRun::from_model(
            run,
            pipeline.slug,
            workspace.slug,
            trigger,
        )),
    ))
}

fn create_pipeline_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start a run (detect)")
        .description(
            "Analyzes a file with the pipeline's configuration and returns the run \
             holding the findings for review. Accepts an Idempotency-Key header.",
        )
        .response::<201, Json<PipelineRun>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists runs for a specific pipeline.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        pipeline_slug = %path_params.pipeline_slug,
    )
)]
async fn list_pipeline_runs(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelinePathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<PipelineRunsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipeline runs");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let pipeline = find_pipeline(&mut conn, workspace.id, &path_params.pipeline_slug).await?;

    let page = conn
        .cursor_list_workspace_pipeline_runs(pipeline.id, pagination.into(), None)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = page.items.len(),
        "Pipeline runs listed"
    );

    let response = PipelineRunsPage::from_cursor_page(page, |wc| {
        PipelineRun::from_model(
            wc.item,
            pipeline.slug.clone(),
            workspace.slug.clone(),
            wc.account.into(),
        )
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_pipeline_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipeline runs")
        .description("Returns all runs for a specific pipeline.")
        .response::<200, Json<PipelineRunsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists all runs across the workspace's pipelines.
///
/// Aggregates runs from every pipeline in the workspace, most recent first,
/// with optional status and pipeline filters. Requires `ViewPipelines`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_workspace_runs(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceRunsQuery>,
) -> Result<(StatusCode, Json<PipelineRunsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace runs");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let page = conn
        .cursor_list_workspace_runs(workspace.id, pagination.into(), query.status)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = page.items.len(),
        "Workspace runs listed"
    );

    Ok((
        StatusCode::OK,
        Json(PipelineRunsPage::from_cursor_page(
            page,
            |(wc, pipeline_slug)| {
                PipelineRun::from_model(
                    wc.item,
                    pipeline_slug,
                    workspace.slug.clone(),
                    wc.account.into(),
                )
            },
        )),
    ))
}

fn list_workspace_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace runs")
        .description(
            "Returns all pipeline runs across the workspace, most recent first, \
             with optional status and pipeline filters.",
        )
        .response::<200, Json<PipelineRunsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Gets a specific pipeline run.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        run_id = %path_params.run_id,
    )
)]
async fn get_pipeline_run(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, Json<PipelineRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting pipeline run");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let (run, pipeline) =
        find_pipeline_run(&mut conn, workspace.id, path_params.run_id.as_uuid()).await?;

    let trigger = resolve_account_ref(&mut conn, run.account_id).await?;

    tracing::debug!(target: TRACING_TARGET, "Pipeline run retrieved");

    Ok((
        StatusCode::OK,
        Json(PipelineRun::from_model(
            run,
            pipeline.slug,
            workspace.slug,
            trigger,
        )),
    ))
}

fn get_pipeline_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get pipeline run")
        .description("Returns the run and its status for review.")
        .response::<200, Json<PipelineRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Redacts a run using the reviewer-verified findings, storing the result.
///
/// Consumes the analyzed run (which must be awaiting review), applies the
/// pipeline's policies to the verified findings, stores the redacted bytes as a
/// new file, and completes the run. Requires `RunPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        run_id = %path_params.run_id,
    )
)]
async fn redact_pipeline_run(
    State(pg_client): State<PgClient>,
    State(blob): State<BlobService>,
    State(crypto): State<CryptoService>,
    State(engine): State<EngineService>,
    State(webhook_emitter): State<WebhookEmitter>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, Json<PipelineRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Redacting pipeline run");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::RunPipelines)
        .await?;

    let (run, pipeline) =
        find_pipeline_run(&mut conn, workspace.id, path_params.run_id.as_uuid()).await?;

    // A run can only be redacted once, after detection.
    if !run.is_analyzed() {
        return Err(ErrorKind::Conflict
            .with_message("Run is not awaiting redaction")
            .with_resource("pipeline_run"));
    }

    // The source document is normally held back from retention while the run is
    // unfinished (see files_due_for_expiry), so this is reachable only if the
    // input was explicitly deleted; surface a message that names the cause.
    let file = conn
        .find_file_in_workspace(workspace.id, run.input_file_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::Conflict
                .with_message("The run's source document is no longer available")
                .with_resource("pipeline_run")
        })?;

    // The stored analysis is the source of truth for what gets redacted. It
    // carries the scope (label catalog) analyze resolved, so redaction compiles
    // against the same vocabulary without re-deriving it.
    let mut analyzed = blob
        .load_analyzed_document(&mut conn, workspace.id, &run)
        .await?;
    let policies = resolve_policies(&mut conn, &crypto, workspace.id, pipeline.id).await?;
    let document = blob.build_document(&file, run.id).await?;

    let redacted = engine.anonymize(document, &policies, &mut analyzed).await?;

    // Store the redacted bytes as a new workspace file and link it to the run.
    let workspace_settings = WorkspaceSettings::from_value(&workspace.settings).retention;
    let output_file = blob
        .store_redacted_file(
            &mut conn,
            &file,
            &pipeline,
            &workspace_settings,
            auth_state.account_id,
            redacted.bytes,
        )
        .await?;

    let run = conn
        .update_workspace_pipeline_run(
            run.id,
            UpdateWorkspacePipelineRun {
                status: Some(PipelineRunStatus::Completed),
                output_file_id: Some(Some(output_file.id)),
                completed_at: Some(Some(jiff::Timestamp::now().into())),
                ..Default::default()
            },
        )
        .await?;

    if let Err(err) = webhook_emitter
        .emit_pipeline_run_completed(
            workspace.id,
            run.id,
            Some(auth_state.account_id),
            Some(serde_json::json!({ "outputFileId": output_file.id })),
        )
        .await
    {
        tracing::warn!(
            target: TRACING_TARGET,
            error = %err,
            run_id = %run.id,
            "Failed to emit pipeline:run.completed webhook event"
        );
    }

    tracing::info!(
        target: TRACING_TARGET,
        run_id = %run.id,
        output_file_id = %output_file.id,
        "Pipeline run redacted"
    );

    let trigger = resolve_account_ref(&mut conn, run.account_id).await?;

    Ok((
        StatusCode::OK,
        Json(PipelineRun::from_model(
            run,
            pipeline.slug,
            workspace.slug,
            trigger,
        )),
    ))
}

fn redact_pipeline_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Redact a run")
        .description(
            "Applies the pipeline's policies to the run's stored analysis, stores \
             the redacted file, and completes the run.",
        )
        .response::<200, Json<PipelineRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Marks a run failed (best effort) after an engine error and emits the
/// `pipeline:run.failed` webhook event.
async fn fail_run(
    conn: &mut PgConn,
    webhook_emitter: &WebhookEmitter,
    workspace_id: uuid::Uuid,
    run_id: uuid::Uuid,
    triggered_by: uuid::Uuid,
) {
    let update = UpdateWorkspacePipelineRun {
        status: Some(PipelineRunStatus::Failed),
        completed_at: Some(Some(jiff::Timestamp::now().into())),
        ..Default::default()
    };
    if let Err(err) = conn.update_workspace_pipeline_run(run_id, update).await {
        tracing::warn!(target: TRACING_TARGET, error = %err, "Failed to mark run failed");
    }

    if let Err(err) = webhook_emitter
        .emit_pipeline_run_failed(workspace_id, run_id, Some(triggered_by), None)
        .await
    {
        tracing::warn!(
            target: TRACING_TARGET,
            error = %err,
            run_id = %run_id,
            "Failed to emit pipeline:run.failed webhook event"
        );
    }
}

/// Maps a definition (de)serialization failure to an internal error.
fn serialize_error(error: serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to process pipeline definition")
        .with_context(error.to_string())
}

/// Finds a pipeline within a workspace by slug or returns NotFound.
async fn find_pipeline(
    conn: &mut PgConn,
    workspace_id: Uuid,
    pipeline_slug: &str,
) -> Result<WorkspacePipeline> {
    conn.find_pipeline_in_workspace_by_slug(workspace_id, pipeline_slug)
        .await?
        .map(|wc| wc.item)
        .ok_or_else(|| Error::not_found("pipeline"))
}

/// Resolves a run by its opaque id within a workspace, returning the run and its
/// owning pipeline (for the response's pipeline slug). The lookup is
/// workspace-scoped through the owning pipeline.
pub(super) async fn find_pipeline_run(
    conn: &mut PgConn,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<(WorkspacePipelineRun, WorkspacePipeline)> {
    conn.find_workspace_run_by_id(workspace_id, run_id)
        .await?
        .ok_or_else(|| Error::not_found("pipeline_run"))
}

/// Returns a [`Router`] with all pipeline run routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/pipelines/runs/",
            get_with(list_workspace_runs, list_workspace_runs_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/pipelines/{pipelineSlug}/runs/",
            post_with(create_pipeline_run, create_pipeline_run_docs)
                .get_with(list_pipeline_runs, list_pipeline_runs_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/runs/{runId}/",
            get_with(get_pipeline_run, get_pipeline_run_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/runs/{runId}/redactions/",
            post_with(redact_pipeline_run, redact_pipeline_run_docs),
        )
        .with_path_items(|item| item.tag("Pipeline Runs"))
}

/// Resolves a pipeline's live policy references into decrypted engine policies.
async fn resolve_policies(
    conn: &mut PgConn,
    crypto: &CryptoService,
    workspace_id: Uuid,
    pipeline_id: Uuid,
) -> Result<Vec<PolicyDefinition>> {
    let ids = conn.list_pipeline_policy_ids(pipeline_id).await?;
    let mut policies = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(model) = conn.find_policy_in_workspace(workspace_id, id).await? {
            policies
                .push(crypto.decrypt_json::<PolicyDefinition>(workspace_id, &model.definition)?);
        }
    }
    Ok(policies)
}
