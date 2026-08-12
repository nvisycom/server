//! Pipeline run handlers: detect, review, and redact.
//!
//! A run is one analysis of a file through a pipeline. Detect creates the run
//! and stores the findings; the run then awaits reviewer verification before
//! redact consumes the verified findings and produces a redacted file.

use std::convert::Infallible;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use async_stream::stream;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::{Stream, StreamExt};
use nvisy_postgres::model::{
    NewWorkspacePipelineRun, UpdateWorkspacePipelineRun, WorkspacePipeline, WorkspacePipelineRun,
};
use nvisy_postgres::query::{
    PipelineReferenceRepository, RunFiles, WorkspaceFileRepository, WorkspacePipelineRepository,
    WorkspacePipelineRunRepository,
};
use nvisy_postgres::types::{PipelineRunStatus, WorkspaceSettings};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, IdempotencyKey, Json, Path, Permission, Query, ValidateJson,
    WorkspaceContext,
};
use crate::handler::request::{
    CreatePipelineRun, CursorPagination, PipelineDefinition, PipelinePathParams,
    PipelineRunPathParams, PipelineRunsQuery, WorkspaceRunsQuery,
};
use crate::handler::response::{
    ErrorResponse, NotificationPayload, PipelineRun, PipelineRunCompletedParams, PipelineRunsPage,
};
use crate::handler::utility::{SseResponse, resolve_account_ref};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{
    CryptoService, DetectionJob, DetectionQueue, EngineService, NotificationEmitter, RunBlobStore,
    RunStatusEvent, ServiceState, WebhookEmitter, fail_run, resolve_policies,
};

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
    State(detection): State<DetectionQueue>,
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
        let files = conn.run_file_names(workspace.id, &existing).await?;
        return Ok((
            StatusCode::OK,
            Json(PipelineRun::from_model(
                existing,
                pipeline.slug.clone(),
                workspace.slug.clone(),
                trigger,
                files,
            )),
        ));
    }

    // Validate synchronously so a bad request fails fast (4xx) rather than as a
    // run that immediately fails in the worker.
    let file = conn
        .find_file_in_workspace(pipeline.workspace_id, request.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;

    if conn.list_pipeline_policy_ids(pipeline.id).await?.is_empty() {
        return Err(ErrorKind::BadRequest
            .with_message("Pipeline has no policies; attach at least one before running")
            .with_resource("pipeline"));
    }

    // Decode the pipeline definition now so an undecodable definition fails the
    // request synchronously (400) instead of returning 202 and failing later in
    // the worker. The decoded value is rebuilt in the worker from the same
    // stored bytes; this is validation only.
    let _validated = PipelineDefinition::from_parts(pipeline.definition.clone(), Vec::new())
        .map_err(|err| {
            ErrorKind::BadRequest
                .with_message("Pipeline definition is invalid")
                .with_resource("pipeline")
                .with_context(err.to_string())
        })?;

    // Create the run (its id is the engine correlation id) and enqueue detection
    // for the worker. The response returns immediately; the client learns the
    // findings are ready via the run's status (SSE at `.../events` or a re-read).
    let new_run = NewWorkspacePipelineRun {
        pipeline_id: pipeline.id,
        input_file_id: file.id,
        account_id: auth_state.account_id,
        status: Some(PipelineRunStatus::Queued),
        idempotency_key: idempotency_key.clone(),
        ..Default::default()
    };
    let run = conn.create_workspace_pipeline_run(new_run).await?;

    let job = DetectionJob {
        workspace_id: workspace.id,
        run_id: run.id,
        scope: request.scope,
    };
    if let Err(err) = detection.enqueue(job).await {
        // Enqueue failed, so the worker will never pick this run up: fail it now
        // rather than leaving it stuck in `Queued`.
        fail_run(
            &mut conn,
            &detection,
            &webhook_emitter,
            workspace.id,
            run.id,
            auth_state.account_id,
            "Failed to enqueue detection",
        )
        .await;
        return Err(err);
    }

    detection
        .broadcast_status(run.id, PipelineRunStatus::Queued)
        .await;

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

    tracing::info!(target: TRACING_TARGET, run_id = %run.id, "Pipeline run enqueued for detection");

    let trigger = resolve_account_ref(&mut conn, run.account_id).await?;

    // The run was just created from this file and has no output yet.
    let files = RunFiles {
        input: Some(file.display_name),
        output: None,
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(PipelineRun::from_model(
            run,
            pipeline.slug,
            workspace.slug,
            trigger,
            files,
        )),
    ))
}

fn create_pipeline_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start a run (detect)")
        .description(
            "Starts detection for a file and returns 202 with the run in the \
             `running` state; the analysis runs in the background. Watch the run's \
             status via the SSE stream at `.../runs/{runId}/events` (or re-read the \
             run) and fetch the findings from `.../runs/{runId}/detections/` once it \
             reaches `analyzed`. A repeated Idempotency-Key returns the existing run.",
        )
        .response::<202, Json<PipelineRun>>()
        .response::<200, Json<PipelineRun>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
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
    Query(query): Query<PipelineRunsQuery>,
) -> Result<(StatusCode, Json<PipelineRunsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipeline runs");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let pipeline = find_pipeline(&mut conn, workspace.id, &path_params.pipeline_slug).await?;

    let page = conn
        .cursor_list_workspace_pipeline_runs(pipeline.id, pagination.into(), &query.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = page.items.len(),
        "Pipeline runs listed"
    );

    let response = PipelineRunsPage::from_cursor_page(page, |row| {
        PipelineRun::from_model(
            row.run,
            row.pipeline_slug,
            workspace.slug.clone(),
            row.account.into(),
            RunFiles {
                input: row.input_file_name,
                output: None,
            },
        )
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_pipeline_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipeline runs")
        .description(
            "Returns runs for a specific pipeline, most recent first, with \
             optional status, file, trigger-account, and trigger-type filters.",
        )
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
        .cursor_list_workspace_runs(workspace.id, pagination.into(), &query.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = page.items.len(),
        "Workspace runs listed"
    );

    Ok((
        StatusCode::OK,
        Json(PipelineRunsPage::from_cursor_page(page, |row| {
            PipelineRun::from_model(
                row.run,
                row.pipeline_slug,
                workspace.slug.clone(),
                row.account.into(),
                RunFiles {
                    input: row.input_file_name,
                    output: None,
                },
            )
        })),
    ))
}

fn list_workspace_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace runs")
        .description(
            "Returns all pipeline runs across the workspace, most recent first, \
             with optional status, file, pipeline, trigger-account, and \
             trigger-type filters.",
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
    let files = conn.run_file_names(workspace.id, &run).await?;

    tracing::debug!(target: TRACING_TARGET, "Pipeline run retrieved");

    Ok((
        StatusCode::OK,
        Json(PipelineRun::from_model(
            run,
            pipeline.slug,
            workspace.slug,
            trigger,
            files,
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

/// Streams a run's status changes as Server-Sent Events until detection settles.
///
/// Emits one `status` event with the run's current status immediately (so a
/// client that connects after detection already finished still learns the
/// state), then forwards each status change. The stream ends once the run leaves
/// the detecting phase (`queued`/`analyzing`) — i.e. detection has produced
/// `analyzed`, or the run `failed`/`cancelled`.
///
/// Live status changes arrive over a best-effort core-NATS broadcast; if none
/// arrives within a short interval the authoritative run row is re-read from the
/// database, so a dropped broadcast never leaves the stream hanging.
///
/// Authenticated like every other route (Bearer); browsers should consume it via
/// a `fetch` stream rather than the native `EventSource`, which cannot send an
/// `Authorization` header.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        run_id = %path_params.run_id,
    )
)]
async fn stream_pipeline_run_events(
    State(pg_client): State<PgClient>,
    State(detection): State<DetectionQueue>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<SseResponse<impl Stream<Item = std::result::Result<Event, Infallible>>, RunStatusEvent>>
{
    tracing::debug!(target: TRACING_TARGET, "Opening run status stream");

    let run_id = path_params.run_id.as_uuid();
    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    // Subscribe BEFORE reading the current status: core-NATS broadcasts are not
    // replayed, so a terminal status published between the read and the
    // subscription going live would otherwise be lost and the stream would hang.
    let mut updates = detection.subscribe_status(run_id).await?;

    // Confirm the run exists (and is workspace-scoped) so a bad id 404s here
    // rather than opening an empty stream.
    let (run, _pipeline) = find_pipeline_run(&mut conn, workspace.id, run_id).await?;
    let current = run.status;
    drop(conn);

    let stream = stream! {
        // Emit the current status first: covers the race where detection settled
        // before the subscription was live (no live event would ever arrive).
        yield Ok(status_event(&RunStatusEvent { run_id, status: current }));
        if !current.is_detecting() {
            return;
        }

        loop {
            match tokio::time::timeout(STATUS_POLL_INTERVAL, updates.next()).await {
                // A live broadcast arrived; forward it and stop once detection settles.
                Ok(Some(event)) => {
                    let settled = !event.status.is_detecting();
                    yield Ok(status_event(&event));
                    if settled {
                        break;
                    }
                }
                // The subscription ended; fall back to the DB so the client still
                // learns the final status.
                Ok(None) => {
                    if let Some(status) = reread_run_status(&pg_client, workspace.id, run_id).await {
                        yield Ok(status_event(&RunStatusEvent { run_id, status }));
                    }
                    break;
                }
                // No broadcast within the interval: re-read the authoritative run
                // row. This recovers a dropped best-effort broadcast (core NATS is
                // at-most-once) instead of hanging on keep-alive forever.
                Err(_) => {
                    if let Some(status) = reread_run_status(&pg_client, workspace.id, run_id).await {
                        yield Ok(status_event(&RunStatusEvent { run_id, status }));
                        if !status.is_detecting() {
                            break;
                        }
                    }
                }
            }
        }
    };

    Ok(SseResponse::new(
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
}

/// How long the status stream waits for a live broadcast before re-reading the
/// authoritative run row from the database (the fallback for a dropped
/// best-effort broadcast).
const STATUS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);

/// Re-reads a run's current status from the database, returning `None` if the
/// run can no longer be read (missing, or a transient error — the next poll
/// retries).
async fn reread_run_status(
    pg_client: &PgClient,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Option<PipelineRunStatus> {
    let mut conn = pg_client.get_connection().await.ok()?;
    match find_pipeline_run(&mut conn, workspace_id, run_id).await {
        Ok((run, _pipeline)) => Some(run.status),
        Err(err) => {
            tracing::debug!(target: TRACING_TARGET, error = %err, %run_id, "Failed to re-read run status");
            None
        }
    }
}

/// OpenAPI documentation for the run status SSE stream.
fn stream_pipeline_run_events_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Stream pipeline run status")
        .description(
            "Opens a Server-Sent Events stream of the run's status changes. \
             Emits the current status immediately, then each transition, and \
             ends once the run settles (analyzed, failed, or cancelled). Each \
             event's `data` is a `RunStatusEvent` (see the response schema). \
             Authenticate with a Bearer token via a `fetch`-based client; the \
             native `EventSource` cannot send an `Authorization` header.",
        )
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Builds a `status` SSE event carrying the run's status change.
fn status_event(event: &RunStatusEvent) -> Event {
    Event::default()
        .event("status")
        .json_data(event)
        .unwrap_or_else(|_| Event::default().event("status"))
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
    State(blob): State<RunBlobStore>,
    State(crypto): State<CryptoService>,
    State(engine): State<EngineService>,
    State(webhook_emitter): State<WebhookEmitter>,
    State(notification_emitter): State<NotificationEmitter>,
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
    let files = conn.run_file_names(workspace.id, &run).await?;

    // Notify the run's trigger that redaction completed (best-effort).
    if let Err(err) = notification_emitter
        .notify_account(
            workspace.id,
            run.account_id,
            NotificationPayload::PipelineRunCompleted(PipelineRunCompletedParams {
                run_id: run.id,
                pipeline_slug: pipeline.slug.to_string(),
                input_file_name: files.input.clone(),
            }),
        )
        .await
    {
        tracing::warn!(target: TRACING_TARGET, error = %err, run_id = %run.id, "Failed to create run-completed notification");
    }

    Ok((
        StatusCode::OK,
        Json(PipelineRun::from_model(
            run,
            pipeline.slug,
            workspace.slug,
            trigger,
            files,
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
            "/workspaces/{workspaceSlug}/runs/{runId}/events",
            get_with(stream_pipeline_run_events, stream_pipeline_run_events_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/runs/{runId}/redactions/",
            post_with(redact_pipeline_run, redact_pipeline_run_docs),
        )
        .with_path_items(|item| item.tag("Pipeline Runs"))
}
