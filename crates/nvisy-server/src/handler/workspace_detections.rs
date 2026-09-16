//! Detection handlers: detect, review, and redact.
//!
//! A detection is one analysis of a document through a pipeline (or, ad-hoc,
//! against an explicit policy list). Create and read delegate to
//! [`WorkspaceDetectionService`]; the async analysis runs in the detection worker.
//! Once a detection is complete a redaction consumes the findings (with optional
//! reviewer edits) and produces a redacted document. A detection can be redacted
//! many times. The streaming (SSE) and redaction actions stay here, loading a
//! detection through the service.

// Axum handlers take their dependencies as typed extractor arguments, so a
// dependency-heavy handler exceeds the arg limit by construction; the signature
// is fixed by the framework and cannot be bundled into a struct.
#![allow(clippy::too_many_arguments)]

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use async_stream::stream;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::Event;
use elide_pipeline::governance::policy::Policy;
use futures::StreamExt;
use nvisy_postgres::model::{
    Blob, NewWorkspaceDocument, NewWorkspaceRedaction,
    WorkspaceDetection as WorkspaceDetectionModel, WorkspaceDocument, WorkspacePipeline,
};
use nvisy_postgres::query::{
    DetectionDocuments, WorkspaceBlobRepository, WorkspaceDocumentRepository,
    WorkspaceRedactionRepository,
};
use nvisy_postgres::types::{DetectionStatus, DocumentKind};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain;
use crate::extract::{
    Authorized, IdempotencyKey, Json, Path, Query, SecurityContext, ValidateJson, markers,
};
use crate::handler::request::{
    CreateAdhocWorkspaceDetection, CreateWorkspaceDetection, CursorPagination,
    RedactWorkspaceDetection, WorkspaceDetectionPathParams, WorkspaceDetectionsQuery,
    WorkspacePipelineDetectionsQuery, WorkspacePipelinePathParams,
};
use crate::handler::response::{
    WorkspaceDetection, WorkspaceDetectionsPage, WorkspaceRedactionResult,
};
use crate::handler::utility::resolve_account_ref;
use crate::response::{Error, ErrorKind, ErrorResponse, Result, SseResponse};
use crate::service::event::EventEmitter;
use crate::service::{
    ArtifactReader, ArtifactWriter, DetectionQueue, EngineService, ServiceState, event,
};
use crate::worker::detection::DetectionStatusEvent;

/// Tracing target for detection operations.
const TRACING_TARGET: &str = "nvisy_server::handler::detections";

/// Maps a service's created/replayed detection to the HTTP response, resolving
/// the triggering account to its public reference.
async fn created_response(
    conn: &mut PgConn,
    created: domain::output::CreatedDetection,
    workspace_id: uuid::Uuid,
    workspace_handle: nvisy_postgres::types::Handle,
) -> Result<(StatusCode, Json<WorkspaceDetection>)> {
    let status = if created.created {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    let trigger = resolve_account_ref(conn, created.trigger_account_id).await?;
    Ok((
        status,
        Json(WorkspaceDetection::from_model(
            &created.detection,
            workspace_id,
            workspace_handle,
            trigger,
            created.documents,
        )),
    ))
}

/// Starts a detection: analyzes a document with the pipeline's configuration.
///
/// Returns the detection holding the findings for review. A repeated request with
/// the same `Idempotency-Key` returns the existing detection instead of analyzing
/// again. Requires `RunDetections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn create_detection(
    State(pg_client): State<PgClient>,
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::RunDetections>,
    Path(path_params): Path<WorkspacePipelinePathParams>,
    IdempotencyKey(idempotency_key): IdempotencyKey,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspaceDetection>,
) -> Result<(StatusCode, Json<WorkspaceDetection>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting detection");

    let workspace = authz.workspace;
    let origin = event::EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };
    let created = detections
        .create(
            origin,
            path_params.pipeline_id,
            idempotency_key,
            request.into(),
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    created_response(&mut conn, created, workspace.id, workspace.handle).await
}

fn create_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start a detection")
        .description(
            "Starts analysis for a document and returns 202 with the detection in the \
             `pending` state; the analysis runs in the background. Watch the \
             detection's status via the SSE stream at \
             `.../detections/{detectionId}/events` (or re-read the detection) and \
             fetch the findings from `.../detections/{detectionId}/analysis/` once \
             it reaches `complete`. A repeated Idempotency-Key returns the existing \
             detection.",
        )
        .response::<202, Json<WorkspaceDetection>>()
        .response::<200, Json<WorkspaceDetection>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Starts an ad-hoc detection: analyzes a document against an explicit list of
/// policies, with no pipeline.
///
/// Returns the detection holding the findings for review. A repeated request with
/// the same `Idempotency-Key` returns the existing detection instead of analyzing
/// again. Requires `RunDetections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn create_adhoc_detection(
    State(pg_client): State<PgClient>,
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::RunDetections>,
    IdempotencyKey(idempotency_key): IdempotencyKey,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateAdhocWorkspaceDetection>,
) -> Result<(StatusCode, Json<WorkspaceDetection>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting ad-hoc detection");

    let workspace = authz.workspace;
    let origin = event::EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };
    let created = detections
        .create_adhoc(origin, idempotency_key, request.into())
        .await?;

    let mut conn = pg_client.get_connection().await?;
    created_response(&mut conn, created, workspace.id, workspace.handle).await
}

fn create_adhoc_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start an ad-hoc detection")
        .description(
            "Starts a detection over a document against an explicit list of policies, \
             with no pipeline. Returns 202 with the pending detection; fetch the findings \
             from `.../detections/{detectionId}/analysis/` once it reaches `complete`. A \
             repeated Idempotency-Key returns the existing detection.",
        )
        .response::<202, Json<WorkspaceDetection>>()
        .response::<200, Json<WorkspaceDetection>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists detections for a specific pipeline.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn list_pipeline_detections(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::ViewDetections>,
    Path(path_params): Path<WorkspacePipelinePathParams>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspacePipelineDetectionsQuery>,
) -> Result<(StatusCode, Json<WorkspaceDetectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipeline detections");

    let workspace = authz.workspace;
    let page = detections
        .list_for_pipeline(
            workspace.id,
            path_params.pipeline_id,
            pagination.into_cursor(),
            &query.into(),
        )
        .await?;

    let response = WorkspaceDetectionsPage::from_cursor_page(page, |row| {
        WorkspaceDetection::from_model(
            &row.detection,
            workspace.id,
            workspace.handle.clone(),
            row.account.into(),
            DetectionDocuments {
                input: row.input_document_name,
            },
        )
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_pipeline_detections_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipeline detections")
        .description(
            "Returns detections for a specific pipeline, most recent first, with \
             optional status, document, trigger-account, and trigger-type filters.",
        )
        .response::<200, Json<WorkspaceDetectionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists all detections across the workspace's pipelines.
///
/// Aggregates detections from every pipeline in the workspace, most recent first,
/// with optional status and pipeline filters. Requires `ViewDetections`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_workspace_detections(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::ViewDetections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceDetectionsQuery>,
) -> Result<(StatusCode, Json<WorkspaceDetectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace detections");

    let workspace = authz.workspace;
    let page = detections
        .list_for_workspace(workspace.id, pagination.into_cursor(), &query.into())
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceDetectionsPage::from_cursor_page(page, |row| {
            WorkspaceDetection::from_model(
                &row.detection,
                workspace.id,
                workspace.handle.clone(),
                row.account.into(),
                DetectionDocuments {
                    input: row.input_document_name,
                },
            )
        })),
    ))
}

fn list_workspace_detections_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace detections")
        .description(
            "Returns all detections across the workspace, most recent first, \
             with optional status, document, pipeline, trigger-account, and \
             trigger-type filters.",
        )
        .response::<200, Json<WorkspaceDetectionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Gets a specific detection.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn get_detection(
    State(pg_client): State<PgClient>,
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::ViewDetections>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
) -> Result<(StatusCode, Json<WorkspaceDetection>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting detection");

    let workspace = authz.workspace;
    let (detection, trigger_account_id, documents) = detections
        .get(workspace.id, path_params.detection_id.as_uuid())
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let trigger = resolve_account_ref(&mut conn, trigger_account_id).await?;

    tracing::debug!(target: TRACING_TARGET, "WorkspaceDetection retrieved");

    Ok((
        StatusCode::OK,
        Json(WorkspaceDetection::from_model(
            &detection,
            workspace.id,
            workspace.handle,
            trigger,
            documents,
        )),
    ))
}

fn get_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get detection")
        .description("Returns the detection and its status for review.")
        .response::<200, Json<WorkspaceDetection>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
async fn stream_detection_events(
    State(detections): State<domain::WorkspaceDetectionService>,
    State(detection): State<DetectionQueue>,
    authz: Authorized<markers::ViewDetections>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
) -> Result<SseResponse<DetectionStatusEvent>> {
    tracing::debug!(target: TRACING_TARGET, "Opening detection status stream");

    let workspace = authz.workspace;
    let detection_id = path_params.detection_id.as_uuid();

    // Subscribe BEFORE reading the current status: core-NATS broadcasts are not
    // replayed, so a terminal status published between the read and the
    // subscription going live would otherwise be lost and the stream would hang.
    let mut updates = detection.subscribe_status(detection_id).await?;

    // Confirm the detection exists (and is workspace-scoped) so a bad id 404s here
    // rather than opening an empty stream.
    let (detection_row, _pipeline) = detections.find(workspace.id, detection_id).await?;
    let current = detection_row.status;

    let workspace_id = workspace.id;
    let stream = stream! {
        // Status progression is monotonic (pending -> executing -> terminal), but
        // its sources are not ordered: this instance broadcasts `Pending` while a
        // worker (possibly on another instance, woken by another instance's
        // drainer) broadcasts `Executing`, and the periodic DB re-read can observe
        // either. Track the furthest phase emitted and drop any event that would
        // move a watcher backwards, so a late `Pending` after `Executing` is never
        // forwarded.
        let mut max_phase = current.phase();

        // Emit the current status first: covers the race where analysis settled
        // before the subscription was live (no live event would ever arrive).
        yield status_event(&DetectionStatusEvent { detection_id, status: current });
        if !current.is_detecting() {
            return;
        }

        loop {
            match tokio::time::timeout(STATUS_POLL_INTERVAL, updates.next()).await {
                // A live broadcast arrived; forward it (unless it moves backwards)
                // and stop once it settles.
                Ok(Some(event)) => {
                    if event.status.phase() < max_phase {
                        continue;
                    }
                    max_phase = event.status.phase();
                    let settled = !event.status.is_detecting();
                    yield status_event(&event);
                    if settled {
                        break;
                    }
                }
                // The subscription ended; fall back to the DB so the client still
                // learns the final status.
                Ok(None) => {
                    if let Some(status) = reread_detection_status(&detections, workspace_id, detection_id).await
                        && status.phase() >= max_phase
                    {
                        yield status_event(&DetectionStatusEvent { detection_id, status });
                    }
                    break;
                }
                // No broadcast within the interval: re-read the authoritative
                // detection row. This recovers a dropped best-effort broadcast
                // (core NATS is at-most-once) instead of hanging on keep-alive.
                Err(_) => {
                    if let Some(status) = reread_detection_status(&detections, workspace_id, detection_id).await
                        && status.phase() >= max_phase
                    {
                        max_phase = status.phase();
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
    detections: &domain::WorkspaceDetectionService,
    workspace_id: Uuid,
    detection_id: Uuid,
) -> Option<DetectionStatus> {
    match detections.find(workspace_id, detection_id).await {
        Ok((detection, _pipeline)) => Some(detection.status),
        Err(err) => {
            tracing::debug!(target: TRACING_TARGET, error = %err, %detection_id, "Failed to re-read detection status");
            None
        }
    }
}

/// `OpenAPI` documentation for the detection status SSE stream.
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

/// The 409 raised when a redaction's source document (or its bytes) is gone: the
/// document row was deleted, or its blob reference cleared. Named once so the
/// document-lookup and blob-lookup paths surface the same message.
fn source_document_gone() -> Error<'static> {
    ErrorKind::Conflict.with_message("The detection's source document is no longer available")
}

/// The pre-flight inputs a redaction reads under a connection before releasing it
/// for the slow analysis and staging work.
struct RedactInputs {
    detection: WorkspaceDetectionModel,
    /// The detection's owning pipeline, if it still has one (for the event handle).
    pipeline: Option<WorkspacePipeline>,
    /// The detection's input document.
    document: WorkspaceDocument,
    /// The input document's backing blob (for its extension and byte access).
    source_blob: Blob,
    /// The base analysis's backing blob, loaded to seed the working audit.
    audit_blob: Blob,
    policies: Vec<Policy>,
}

/// Redacts a detection using its findings, storing the result.
///
/// Applies the pipeline's policies to the detection's stored analysis, stores the
/// redacted bytes as a new document, and emits a redaction event. Requires
/// `RunRedactions` permission. A detection can be redacted more than once.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        detection_id = %path_params.detection_id,
    )
)]
// reason: long but cohesive; splitting adds no clarity.
#[allow(clippy::too_many_lines)]
async fn redact_detection(
    State(pg_client): State<PgClient>,
    State(detections): State<domain::WorkspaceDetectionService>,
    State(writer): State<ArtifactWriter>,
    State(reader): State<ArtifactReader>,
    State(engine): State<EngineService>,
    authz: Authorized<markers::RunRedactions>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
    security: SecurityContext,
    Json(request): Json<RedactWorkspaceDetection>,
) -> Result<(StatusCode, Json<WorkspaceRedactionResult>)> {
    tracing::debug!(target: TRACING_TARGET, "Redacting detection");

    let workspace = authz.workspace;

    // Phase 1: the pre-flight DB work under one connection, then release it.
    // Holding a pooled connection across the audit load, redaction inference, and
    // object I/O below would pin it for many seconds and starve the pool under
    // load, so this scope drops the connection before that slow work begins. Only
    // the audit blob is resolved here; its bytes are loaded in phase 2. The
    // detection lookup goes through the service; the rest is redaction's own work.
    let (detection, pipeline) = detections
        .find(workspace.id, path_params.detection_id.as_uuid())
        .await?;

    // Redaction derives from this detection's base audit, so it re-redacts with
    // the exact policy versions the detection pinned when it ran, not the
    // policies' current versions. Resolved through the service (which manages its
    // own connection) before the pre-flight `conn` is acquired, so the two
    // connections never overlap and a burst of redacts cannot deadlock the pool.
    let policies = detections
        .resolve_pinned_policies(workspace.id, detection.id)
        .await?;

    let inputs = {
        let mut conn = pg_client.get_connection().await?;

        // A detection can only be redacted once its analysis is complete.
        if !detection.status.is_complete() {
            return Err(
                ErrorKind::Conflict.with_message("WorkspaceDetection is not ready to redact")
            );
        }

        // The source document is reachable only if it was not explicitly deleted;
        // surface a message that names the cause.
        let document = conn
            .find_document_in_workspace(workspace.id, detection.input_document_id)
            .await?
            .ok_or_else(source_document_gone)?;
        let source_blob = conn
            .find_blob_by_id(document.blob_id.ok_or_else(source_document_gone)?)
            .await?
            .ok_or_else(source_document_gone)?;

        // The detection's base analysis blob, loaded to seed the working audit.
        // A completed detection with a NULL pointer had its bytes reclaimed (404);
        // an incomplete one has no analysis yet (409).
        let audit_blob_id = detection.audit_blob_id.ok_or_else(|| {
            if detection.status.is_complete() {
                ErrorKind::NotFound.with_message("The analysis for this detection has been deleted")
            } else {
                ErrorKind::Conflict.with_message("WorkspaceDetection has no analysis yet")
            }
        })?;
        let audit_blob = conn.find_blob_by_id(audit_blob_id).await?.ok_or_else(|| {
            ErrorKind::NotFound.with_message("The analysis for this detection has been deleted")
        })?;
        RedactInputs {
            detection,
            pipeline,
            document,
            source_blob,
            audit_blob,
            policies,
        }
    };

    // Phase 2: the slow work — loading the analysis, applying reviewer edits, the
    // redaction inference, and staging the produced objects — runs with no DB
    // connection held.

    // The stored detection analysis is loaded into a working audit and never
    // mutated on disk: reviewer edits and the redaction outcome land on this
    // clone, which is persisted as the redaction's own review audit, leaving the
    // detection analysis immutable and re-redactable.
    let mut reviewed = reader
        .load_audit(&engine, workspace.id, &inputs.audit_blob)
        .await?;

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

    let document = writer
        .build_document(&inputs.document, &inputs.source_blob, inputs.detection.id)
        .await?;

    // No per-request key: the server does not yet drive keyed operators
    // (HMAC/encrypt), whose `KeyConfig` would be supplied here. The codec params
    // and document context are read back from the audit, recorded at detect time.
    let redacted = engine
        .anonymize(document, &inputs.policies, &mut reviewed, None)
        .await?;

    // Stage both produced objects (redacted document + review audit) outside the
    // transaction — object writes are not transactional — then commit their blobs,
    // document, audit, and redaction rows together. On rollback the staged objects
    // are reclaimed so no orphaned bytes accrue. Retention comes from the
    // detection's snapshotted override (the run's own retention), not the live
    // pipeline, so re-redacting reflects what the detection ran under.
    let retention = workspace.settings.or_default().retention;
    // The detection's snapshotted override, decoded to an owned value borrowed by
    // both staging calls below.
    let retention_override = inputs
        .detection
        .retention_override
        .as_ref()
        .map(nvisy_postgres::types::Json::or_default);
    let staged_output = writer
        .stage_redacted_document(
            &inputs.document,
            retention_override.as_ref(),
            &retention,
            redacted.bytes,
        )
        .await?;
    // Staging the review audit after the output means a failure here would strand
    // the already-written output object (no blob to reclaim it); discard it first.
    let staged_review = match writer
        .stage_review_audit(
            inputs.detection.workspace_id,
            retention_override.as_ref(),
            &retention,
            &reviewed,
        )
        .await
    {
        Ok(staged) => staged,
        Err(err) => {
            writer.discard_staged_object(&staged_output.0).await.ok();
            return Err(err);
        }
    };

    // Phase 3: re-acquire a connection only for the final commit, so the pool was
    // free during the inference and staging above. A failure here leaves both
    // staged objects with no blob row for the reaper to find, so discard them
    // before returning — as every later failure path does.
    let mut conn = match pg_client.get_connection().await {
        Ok(conn) => conn,
        Err(err) => {
            writer.discard_staged_object(&staged_output.0).await.ok();
            writer.discard_staged_object(&staged_review).await.ok();
            return Err(err.into());
        }
    };
    let redaction = conn
        .transaction(async |conn| {
            // The redacted output is a first-class document (kind=redacted); its
            // blob is resolved (shared or created) as the document is created.
            let output_document = conn
                .create_workspace_document(
                    NewWorkspaceDocument {
                        workspace_id: workspace.id,
                        account_id: authz.account_id,
                        blob_id: Uuid::nil(),
                        kind: Some(DocumentKind::Redacted),
                        display_name: Some(staged_output.1.clone()),
                        original_filename: Some(inputs.document.original_filename.clone()),
                        file_extension: Some(inputs.document.file_extension.clone()),
                        metadata: None,
                    },
                    staged_output.0.clone(),
                )
                .await?;
            // The review analysis is a blob-ref on the redaction: resolve (share or
            // insert) its blob to record the reference, then point the redaction at
            // it via `review_audit_blob_id`.
            let review_resolved = conn.find_or_create_blob(staged_review.clone()).await?;
            let redaction = conn
                .create_redaction(NewWorkspaceRedaction {
                    detection_id: inputs.detection.id,
                    account_id: authz.account_id,
                    output_document_id: Some(output_document.id),
                    review_audit_blob_id: Some(review_resolved.id),
                })
                .await?;
            conn.emit_event(
                event::EventOrigin {
                    workspace_id: workspace.id,
                    account_id: authz.account_id,
                    security: &security,
                },
                event::WorkspaceEvent::RedactionCreated(event::RedactionCreated {
                    detection_id: inputs.detection.id,
                    pipeline_id: inputs.pipeline.as_ref().map(|p| p.id),
                    redaction_id: redaction.id,
                    input_document_name: Some(inputs.document.display_name.clone()),
                    notify: inputs.detection.account_id,
                }),
            )
            .await?;

            // A redaction is just a produced output; it creates and touches no
            // review. A reviewer links this redaction to a review explicitly.
            // The resolved storage paths, so an object staged for content that
            // deduplicated onto an existing blob can be reclaimed after commit.
            // These rows were just inserted with a blob, so the pointers are set;
            // resolve a storage path only when present (a defensive `None` skips it).
            let output_path = match output_document.blob_id {
                Some(blob_id) => conn
                    .find_blob_by_id(blob_id)
                    .await?
                    .map(|blob| blob.storage_path),
                None => None,
            };
            let review_path = Some(review_resolved.storage_path);
            Ok::<_, Error>((redaction, output_path, review_path))
        })
        .await;

    let (redaction, output_path, review_path) = match redaction {
        Ok(committed) => committed,
        Err(err) => {
            // The rows rolled back, so their staged objects are orphans: reclaim
            // both (best effort — a failure only leaves them for a later sweep).
            writer.discard_staged_object(&staged_output.0).await.ok();
            writer.discard_staged_object(&staged_review).await.ok();
            return Err(err);
        }
    };

    // A committed blob whose content deduplicated onto an existing object leaves
    // the object staged for it orphaned (no row references it). Reclaim each
    // redundant staged object; best effort, a failure only defers it to a sweep.
    if output_path.as_deref() != Some(staged_output.0.storage_path.as_str()) {
        writer.discard_staged_object(&staged_output.0).await.ok();
    }
    if review_path.as_deref() != Some(staged_review.storage_path.as_str()) {
        writer.discard_staged_object(&staged_review).await.ok();
    }

    tracing::info!(
        target: TRACING_TARGET,
        detection_id = %inputs.detection.id,
        redaction_id = %redaction.id,
        "WorkspaceDetection redacted"
    );

    let requested_by = resolve_account_ref(&mut conn, redaction.account_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceRedactionResult::from_model(
            &redaction,
            workspace.id,
            workspace.handle,
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
        .response::<201, Json<WorkspaceRedactionResult>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all detection routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/pipelines/detections",
            get_with(list_workspace_detections, list_workspace_detections_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/pipelines/{pipelineId}/detections",
            post_with(create_detection, create_detection_docs)
                .get_with(list_pipeline_detections, list_pipeline_detections_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections",
            post_with(create_adhoc_detection, create_adhoc_detection_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}",
            get_with(get_detection, get_detection_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}/events",
            get_with(stream_detection_events, stream_detection_events_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}/redactions",
            post_with(redact_detection, redact_detection_docs),
        )
        .with_path_items(|item| item.tag("WorkspaceDetections"))
}
