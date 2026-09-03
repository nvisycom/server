//! Detection handlers: detect, review, and redact.
//!
//! A detection is one analysis of a file through a pipeline. Detect creates the
//! detection and stores the findings; once it is complete a redaction consumes
//! the findings (with optional reviewer edits) and produces a redacted file. A
//! detection can be redacted many times.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use async_stream::stream;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::Event;
use futures::StreamExt;
use nvisy_postgres::model::{
    NewWorkspaceDetection, NewWorkspaceDetectionJob, NewWorkspaceRedaction, WorkspaceDetection,
    WorkspacePipeline,
};
use nvisy_postgres::query::{
    DetectionFiles, DetectionJobOutboxRepository, PipelineReferenceRepository,
    WorkspaceDetectionRepository, WorkspaceFileRepository, WorkspacePipelineRepository,
    WorkspaceRedactionRepository,
};
use nvisy_postgres::types::DetectionStatus;
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, IdempotencyKey, Json, Path, Permission, Query, SecurityContext,
    ValidateJson, WorkspaceContext,
};
use crate::handler::request::{
    CreateDetection, CursorPagination, DetectionPathParams, PipelineDefinition,
    PipelineDetectionsQuery, PipelinePathParams, RedactDetection, WorkspaceDetectionsQuery,
};
use crate::handler::response::{Detection, DetectionsPage, ErrorResponse, RedactionResult};
use crate::handler::utility::{SseResponse, resolve_account_ref};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{
    CryptoService, DetectionJob, DetectionQueue, DetectionRef, DetectionStatusEvent, EngineService,
    EventEmitter, EventOrigin, RunBlobStore, ServiceState, WorkspaceEvent, resolve_policies,
};

/// Tracing target for detection operations.
const TRACING_TARGET: &str = "nvisy_server::handler::detections";

/// Starts a detection: analyzes a file with the pipeline's configuration.
///
/// Returns the detection holding the findings for review. A repeated request with
/// the same `Idempotency-Key` returns the existing detection instead of analyzing
/// again. Requires `RunPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        pipeline_slug = %path_params.pipeline_slug,
    )
)]
async fn create_detection(
    State(pg_client): State<PgClient>,
    State(detection): State<DetectionQueue>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelinePathParams>,
    IdempotencyKey(idempotency_key): IdempotencyKey,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateDetection>,
) -> Result<(StatusCode, Json<Detection>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting detection");

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

    // Idempotent replay: a repeated key returns the detection created the first
    // time, attributed to whoever originally triggered it (not the current
    // caller).
    if let Some(key) = &idempotency_key
        && let Some(existing) = conn
            .find_detection_by_idempotency_key(pipeline.id, key)
            .await?
    {
        tracing::debug!(target: TRACING_TARGET, "Replaying detection for idempotency key");
        let trigger = resolve_account_ref(&mut conn, existing.account_id).await?;
        let files = conn.detection_file_names(workspace.id, &existing).await?;
        return Ok((
            StatusCode::OK,
            Json(Detection::from_model(
                existing,
                pipeline.slug.clone(),
                workspace.slug.clone(),
                trigger,
                files,
            )),
        ));
    }

    // Validate synchronously so a bad request fails fast (4xx) rather than as a
    // detection that immediately fails in the worker.
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

    // Create the detection (its id is the engine correlation id). The response
    // returns immediately; the client learns the findings are ready via the
    // detection's status (SSE at `.../events` or a re-read).
    let new_detection = NewWorkspaceDetection {
        pipeline_id: pipeline.id,
        input_file_id: file.id,
        account_id: auth_state.account_id,
        status: Some(DetectionStatus::Pending),
        idempotency_key: idempotency_key.clone(),
        ..Default::default()
    };

    // Create the detection, record its start event, and queue its analysis in one
    // transaction, so all three commit or roll back together. The job goes onto
    // the outbox (not published inline) so the detection is never lost to a
    // publish that failed after the row committed, nor marked failed for a publish
    // that in fact went through: the drainer relays the outbox row to the
    // work-queue, and the worker's claim dedups an at-least-once redelivery.
    let scope = request.scope;
    let detection_row = conn
        .transaction(async |conn| {
            let detection_row = conn.create_workspace_detection(new_detection).await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id: auth_state.account_id,
                    security: &security,
                },
                WorkspaceEvent::DetectionStarted(DetectionRef {
                    detection_id: detection_row.id,
                    pipeline_slug: pipeline.slug.clone(),
                }),
            )
            .await?;
            let job = DetectionJob {
                workspace_id: workspace.id,
                detection_id: detection_row.id,
                scope,
            };
            conn.insert_detection_job(NewWorkspaceDetectionJob {
                detection_id: detection_row.id,
                job: serde_json::to_value(&job).map_err(|err| {
                    ErrorKind::InternalServerError
                        .with_message("Failed to encode detection job")
                        .with_context(err.to_string())
                })?,
            })
            .await?;
            Ok::<_, Error>(detection_row)
        })
        .await?;

    // Best-effort UI hint; the detection row is authoritative.
    detection
        .broadcast_status(detection_row.id, DetectionStatus::Pending)
        .await;

    tracing::info!(target: TRACING_TARGET, detection_id = %detection_row.id, "Detection queued");

    let trigger = resolve_account_ref(&mut conn, detection_row.account_id).await?;

    // The detection was just created from this file.
    let files = DetectionFiles {
        input: Some(file.display_name),
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(Detection::from_model(
            detection_row,
            pipeline.slug,
            workspace.slug,
            trigger,
            files,
        )),
    ))
}

fn create_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start a detection")
        .description(
            "Starts analysis for a file and returns 202 with the detection in the \
             `pending` state; the analysis runs in the background. Watch the \
             detection's status via the SSE stream at \
             `.../detections/{detectionId}/events` (or re-read the detection) and \
             fetch the findings from `.../detections/{detectionId}/analysis/` once \
             it reaches `complete`. A repeated Idempotency-Key returns the existing \
             detection.",
        )
        .response::<202, Json<Detection>>()
        .response::<200, Json<Detection>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Lists detections for a specific pipeline.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        pipeline_slug = %path_params.pipeline_slug,
    )
)]
async fn list_pipeline_detections(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PipelinePathParams>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<PipelineDetectionsQuery>,
) -> Result<(StatusCode, Json<DetectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipeline detections");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let pipeline = find_pipeline(&mut conn, workspace.id, &path_params.pipeline_slug).await?;

    let page = conn
        .cursor_list_pipeline_detections(pipeline.id, pagination.into(), &query.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        detection_count = page.items.len(),
        "Pipeline detections listed"
    );

    let response = DetectionsPage::from_cursor_page(page, |row| {
        Detection::from_model(
            row.detection,
            row.pipeline_slug,
            workspace.slug.clone(),
            row.account.into(),
            DetectionFiles {
                input: row.input_file_name,
            },
        )
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_pipeline_detections_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipeline detections")
        .description(
            "Returns detections for a specific pipeline, most recent first, with \
             optional status, file, trigger-account, and trigger-type filters.",
        )
        .response::<200, Json<DetectionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists all detections across the workspace's pipelines.
///
/// Aggregates detections from every pipeline in the workspace, most recent first,
/// with optional status and pipeline filters. Requires `ViewPipelines`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_workspace_detections(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceDetectionsQuery>,
) -> Result<(StatusCode, Json<DetectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace detections");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let page = conn
        .cursor_list_workspace_detections(workspace.id, pagination.into(), &query.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        detection_count = page.items.len(),
        "Workspace detections listed"
    );

    Ok((
        StatusCode::OK,
        Json(DetectionsPage::from_cursor_page(page, |row| {
            Detection::from_model(
                row.detection,
                row.pipeline_slug,
                workspace.slug.clone(),
                row.account.into(),
                DetectionFiles {
                    input: row.input_file_name,
                },
            )
        })),
    ))
}

fn list_workspace_detections_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace detections")
        .description(
            "Returns all detections across the workspace, most recent first, \
             with optional status, file, pipeline, trigger-account, and \
             trigger-type filters.",
        )
        .response::<200, Json<DetectionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Gets a specific detection.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn get_detection(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<DetectionPathParams>,
) -> Result<(StatusCode, Json<Detection>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting detection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    let (detection, pipeline) =
        find_detection(&mut conn, workspace.id, path_params.detection_id.as_uuid()).await?;

    let trigger = resolve_account_ref(&mut conn, detection.account_id).await?;
    let files = conn.detection_file_names(workspace.id, &detection).await?;

    tracing::debug!(target: TRACING_TARGET, "Detection retrieved");

    Ok((
        StatusCode::OK,
        Json(Detection::from_model(
            detection,
            pipeline.slug,
            workspace.slug,
            trigger,
            files,
        )),
    ))
}

fn get_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get detection")
        .description("Returns the detection and its status for review.")
        .response::<200, Json<Detection>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Streams a detection's status changes as Server-Sent Events until it settles.
///
/// Emits one `status` event with the detection's current status immediately (so a
/// client that connects after analysis already finished still learns the state),
/// then forwards each status change. The stream ends once the detection leaves
/// the detecting phase (`pending`/`executing`) — i.e. analysis has produced
/// `complete`, or the detection `failed`.
///
/// Live status changes arrive over a best-effort core-NATS broadcast; if none
/// arrives within a short interval the authoritative detection row is re-read from
/// the database, so a dropped broadcast never leaves the stream hanging.
///
/// Authenticated like every other route (Bearer); browsers should consume it via
/// a `fetch` stream rather than the native `EventSource`, which cannot send an
/// `Authorization` header.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn stream_detection_events(
    State(pg_client): State<PgClient>,
    State(detection): State<DetectionQueue>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<DetectionPathParams>,
) -> Result<SseResponse<DetectionStatusEvent>> {
    tracing::debug!(target: TRACING_TARGET, "Opening detection status stream");

    let detection_id = path_params.detection_id.as_uuid();
    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPipelines)
        .await?;

    // Subscribe BEFORE reading the current status: core-NATS broadcasts are not
    // replayed, so a terminal status published between the read and the
    // subscription going live would otherwise be lost and the stream would hang.
    let mut updates = detection.subscribe_status(detection_id).await?;

    // Confirm the detection exists (and is workspace-scoped) so a bad id 404s here
    // rather than opening an empty stream.
    let (detection_row, _pipeline) = find_detection(&mut conn, workspace.id, detection_id).await?;
    let current = detection_row.status;
    drop(conn);

    let stream = stream! {
        // Emit the current status first: covers the race where analysis settled
        // before the subscription was live (no live event would ever arrive).
        yield status_event(&DetectionStatusEvent { detection_id, status: current });
        if !current.is_detecting() {
            return;
        }

        loop {
            match tokio::time::timeout(STATUS_POLL_INTERVAL, updates.next()).await {
                // A live broadcast arrived; forward it and stop once it settles.
                Ok(Some(event)) => {
                    let settled = !event.status.is_detecting();
                    yield status_event(&event);
                    if settled {
                        break;
                    }
                }
                // The subscription ended; fall back to the DB so the client still
                // learns the final status.
                Ok(None) => {
                    if let Some(status) = reread_detection_status(&pg_client, workspace.id, detection_id).await {
                        yield status_event(&DetectionStatusEvent { detection_id, status });
                    }
                    break;
                }
                // No broadcast within the interval: re-read the authoritative
                // detection row. This recovers a dropped best-effort broadcast
                // (core NATS is at-most-once) instead of hanging on keep-alive.
                Err(_) => {
                    if let Some(status) = reread_detection_status(&pg_client, workspace.id, detection_id).await {
                        yield status_event(&DetectionStatusEvent { detection_id, status });
                        if !status.is_detecting() {
                            break;
                        }
                    }
                }
            }
        }
    };

    Ok(SseResponse::new(stream))
}

/// How long the status stream waits for a live broadcast before re-reading the
/// authoritative detection row from the database (the fallback for a dropped
/// best-effort broadcast).
const STATUS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);

/// Re-reads a detection's current status from the database, returning `None` if
/// the detection can no longer be read (missing, or a transient error — the next
/// poll retries).
async fn reread_detection_status(
    pg_client: &PgClient,
    workspace_id: Uuid,
    detection_id: Uuid,
) -> Option<DetectionStatus> {
    let mut conn = pg_client.get_connection().await.ok()?;
    match find_detection(&mut conn, workspace_id, detection_id).await {
        Ok((detection, _pipeline)) => Some(detection.status),
        Err(err) => {
            tracing::debug!(target: TRACING_TARGET, error = %err, %detection_id, "Failed to re-read detection status");
            None
        }
    }
}

/// OpenAPI documentation for the detection status SSE stream.
fn stream_detection_events_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Stream detection status")
        .description(
            "Opens a Server-Sent Events stream of the detection's status changes. \
             Emits the current status immediately, then each transition, and \
             ends once the detection settles (complete or failed). Each event's \
             `data` is a `DetectionStatusEvent` (see the response schema). \
             Authenticate with a Bearer token via a `fetch`-based client; the \
             native `EventSource` cannot send an `Authorization` header.",
        )
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Builds a `status` SSE event carrying the detection's status change.
fn status_event(event: &DetectionStatusEvent) -> Event {
    Event::default()
        .event("status")
        .json_data(event)
        .unwrap_or_else(|_| Event::default().event("status"))
}

/// Redacts a detection using its findings, storing the result.
///
/// Applies the pipeline's policies to the detection's stored analysis, stores the
/// redacted bytes as a new file, and emits a redaction event. Requires
/// `RunPipelines` permission. A detection can be redacted more than once.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn redact_detection(
    State(pg_client): State<PgClient>,
    State(blob): State<RunBlobStore>,
    State(crypto): State<CryptoService>,
    State(engine): State<EngineService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<DetectionPathParams>,
    security: SecurityContext,
    Json(request): Json<RedactDetection>,
) -> Result<(StatusCode, Json<RedactionResult>)> {
    tracing::debug!(target: TRACING_TARGET, "Redacting detection");

    // Phase 1: the pre-flight DB work under one connection, then release it.
    // Holding a pooled connection across the audit load, redaction inference, and
    // object I/O below would pin it for many seconds and starve the pool under
    // load, so this scope drops the connection before that slow work begins. Only
    // the audit file row is resolved here; its bytes are loaded in phase 2.
    let (detection, pipeline, file, audit_file, policies) = {
        let mut conn = pg_client.get_connection().await?;

        auth_state
            .authorize_workspace(&mut conn, workspace.id, Permission::RunPipelines)
            .await?;

        let (detection, pipeline) =
            find_detection(&mut conn, workspace.id, path_params.detection_id.as_uuid()).await?;

        // A detection can only be redacted once its analysis is complete.
        if !detection.is_complete() {
            return Err(ErrorKind::Conflict
                .with_message("Detection is not ready to redact")
                .with_resource("detection"));
        }

        // The source document is normally held back from retention while the
        // detection is unfinished (see files_due_for_expiry), so this is reachable
        // only if the input was explicitly deleted; surface a message that names
        // the cause.
        let file = conn
            .find_file_in_workspace(workspace.id, detection.input_file_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::Conflict
                    .with_message("The detection's source document is no longer available")
                    .with_resource("detection")
            })?;

        let audit_file = blob
            .resolve_audit_file(&mut conn, workspace.id, &detection)
            .await?;
        let policies = resolve_policies(&mut conn, &crypto, workspace.id, pipeline.id).await?;

        (detection, pipeline, file, audit_file, policies)
    };

    // Phase 2: the slow work — loading the analysis, applying reviewer edits, the
    // redaction inference, and staging the produced objects — runs with no DB
    // connection held.

    // The stored detection analysis is loaded into a working audit and never
    // mutated on disk: reviewer edits and the redaction outcome land on this
    // clone, which is persisted as the redaction's own review audit, leaving the
    // detection analysis immutable and re-redactable.
    let mut reviewed = blob.load_audit(&engine, workspace.id, &audit_file).await?;

    // Layer the reviewer's edits onto the working audit's report before redaction.
    // Both validation and landing are report-relative (an unknown target, a
    // self-contradiction, or an edit naming a part of the wrong modality → 400, via
    // `EditError`'s `From` impl), so a reviewer is never told a decision took effect
    // when the document says otherwise. `apply` is fallible in its own right: some
    // landing failures (a modality mismatch) surface only when the edit lands.
    if let Some(edits) = &request.edits {
        edits.validate(&reviewed.report)?;
        edits.apply(&mut reviewed.report)?;
    }

    let document = blob.build_document(&file, detection.id).await?;

    // No per-request key: the server does not yet drive keyed operators
    // (HMAC/encrypt), whose `KeyConfig` would be supplied here. The codec params
    // and document context are read back from the audit, recorded at detect time.
    let redacted = engine
        .anonymize(document, &policies, &mut reviewed, None)
        .await?;

    // Stage both produced objects (redacted document + review audit) outside the
    // transaction — object writes are not transactional — then commit their file
    // rows and the redaction row together. On rollback the staged objects are
    // reclaimed so no orphaned bytes accrue.
    let retention = workspace.settings.or_default().retention;
    let staged_output = blob
        .stage_redacted_file(
            &file,
            &pipeline,
            &retention,
            auth_state.account_id,
            redacted.bytes,
        )
        .await?;
    // Staging the review audit after the output means a failure here would strand
    // the already-written output object (no row to reclaim it); discard it first.
    let staged_review = match blob
        .stage_review_audit(&pipeline, &retention, auth_state.account_id, &reviewed)
        .await
    {
        Ok(staged) => staged,
        Err(err) => {
            blob.discard_staged_object(&staged_output).await.ok();
            return Err(err);
        }
    };

    // Phase 3: re-acquire a connection only for the final commit, so the pool was
    // free during the inference and staging above.
    let mut conn = pg_client.get_connection().await?;
    let redaction = conn
        .transaction(async |conn| {
            let output_file = conn.create_workspace_file(staged_output.clone()).await?;
            let review_file = conn.create_workspace_file(staged_review.clone()).await?;
            let redaction = conn
                .create_redaction(NewWorkspaceRedaction {
                    detection_id: detection.id,
                    account_id: auth_state.account_id,
                    review_file_id: Some(review_file.id),
                    output_file_id: Some(output_file.id),
                })
                .await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id: auth_state.account_id,
                    security: &security,
                },
                WorkspaceEvent::RedactionCreated {
                    detection: DetectionRef {
                        detection_id: detection.id,
                        pipeline_slug: pipeline.slug.clone(),
                    },
                    redaction_id: redaction.id,
                    input_file_name: Some(file.display_name.clone()),
                    notify: detection.account_id,
                },
            )
            .await?;
            Ok::<_, Error>(redaction)
        })
        .await;

    let redaction = match redaction {
        Ok(redaction) => redaction,
        Err(err) => {
            // The rows rolled back, so their staged objects are orphans: reclaim
            // both (best effort — a failure only leaves them for a later sweep).
            blob.discard_staged_object(&staged_output).await.ok();
            blob.discard_staged_object(&staged_review).await.ok();
            return Err(err);
        }
    };

    tracing::info!(
        target: TRACING_TARGET,
        detection_id = %detection.id,
        redaction_id = %redaction.id,
        "Detection redacted"
    );

    let requested_by = resolve_account_ref(&mut conn, redaction.account_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(RedactionResult::from_model(
            redaction,
            workspace.slug,
            requested_by,
        )),
    ))
}

fn redact_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Redact a detection")
        .description(
            "Applies the pipeline's policies to the detection's stored analysis — with any \
             reviewer `edits` layered on first (suppress a false positive, retag a detection, or \
             add one the analysis missed) — and produces a new redaction: a redacted document \
             plus a review audit recording what was redacted. A detection can be redacted more \
             than once. An edit targeting a detection not in the analysis, or a set that \
             contradicts itself, is rejected (400).",
        )
        .response::<201, Json<RedactionResult>>()
        .response::<400, Json<ErrorResponse>>()
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

/// Resolves a detection by its opaque id within a workspace, returning the
/// detection and its owning pipeline (for the response's pipeline slug). The
/// lookup is workspace-scoped through the owning pipeline.
pub(super) async fn find_detection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    detection_id: Uuid,
) -> Result<(WorkspaceDetection, WorkspacePipeline)> {
    conn.find_workspace_detection_by_id(workspace_id, detection_id)
        .await?
        .ok_or_else(|| Error::not_found("detection"))
}

/// Returns a [`Router`] with all detection routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/pipelines/detections/",
            get_with(list_workspace_detections, list_workspace_detections_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/pipelines/{pipelineSlug}/detections/",
            post_with(create_detection, create_detection_docs)
                .get_with(list_pipeline_detections, list_pipeline_detections_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/detections/{detectionId}/",
            get_with(get_detection, get_detection_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/detections/{detectionId}/events/",
            get_with(stream_detection_events, stream_detection_events_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/detections/{detectionId}/redactions/",
            post_with(redact_detection, redact_detection_docs),
        )
        .with_path_items(|item| item.tag("Detections"))
}
