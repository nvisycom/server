//! Detection handlers: detect, review, and redact.
//!
//! A detection is one analysis of a document against an explicit policy list. Create and read delegate to
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
use nvisy_postgres::types::DetectionStatus;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain;
use crate::extract::{Authorized, IdempotencyKey, Json, Path, Query, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceDetection, CursorPagination, EditWorkspaceDetection, RedactWorkspaceDetection,
    WorkspaceDetectionPathParams, WorkspaceDetectionsQuery,
};
use crate::handler::response::{
    DetectionTreesPage, WorkspaceDetection, WorkspaceDetectionSummary, WorkspaceDetectionsPage,
    WorkspaceRedactionResult, WorkspaceRef,
};
use crate::handler::utility::RedactionKey;
use crate::response::{ErrorResponse, Result, SseResponse, Watch, sse_event, watch};
use crate::service::{DetectionQueue, ServiceState};
use crate::worker::detection::DetectionStatusEvent;

/// Tracing target for detection operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_detections";

/// Builds a detection's response.
fn detection_response(
    workspace: WorkspaceRef,
    found: domain::output::FoundDetection,
) -> WorkspaceDetection {
    WorkspaceDetection::from_model(
        &found.detection,
        workspace,
        found.trigger.into(),
        found.documents,
    )
}

/// Maps a service's created/replayed detection to the HTTP response: `202` when
/// this call started it, `200` when an idempotency key replayed it.
fn created_response(
    workspace: WorkspaceRef,
    created: domain::output::CreatedDetection,
) -> (StatusCode, Json<WorkspaceDetection>) {
    let status = if created.created {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    (status, Json(detection_response(workspace, created.found)))
}

/// Starts a detection: analyzes a document against an explicit policy list.
///
/// Returns the detection holding the findings for review. A repeated request with
/// the same `Idempotency-Key` returns the existing detection instead of analyzing
/// again. Requires `RunDetections` permission.
#[tracing::instrument(skip_all)]
async fn create_detection(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::RunDetections>,
    IdempotencyKey(idempotency_key): IdempotencyKey,
    ValidateJson(request): ValidateJson<CreateWorkspaceDetection>,
) -> Result<(StatusCode, Json<WorkspaceDetection>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting detection");

    let workspace = &authz.workspace;
    let origin = authz.origin();
    let created = detections
        .create(origin, &authz.account, idempotency_key, request.into())
        .await?;

    Ok(created_response(WorkspaceRef::from(workspace), created))
}

fn create_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start a detection")
        .description(
            "Starts analysis for a document against named policies (a `policies` \
             source) or a list of built-in labels (a `labels` source, run as a derived \
             policy that an identical label list reuses), and returns 202 with the \
             detection in the `pending` state; the analysis runs \
             in the background. Watch the detection's status via the SSE stream at \
             `.../detections/{detectionId}/events` (or re-read the detection) and \
             fetch the findings from `.../detections/{detectionId}/analysis/` once it \
             reaches `complete`. A repeated Idempotency-Key returns the existing \
             detection.",
        )
        .response::<202, Json<WorkspaceDetection>>()
        .response::<200, Json<WorkspaceDetection>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Lists all detections in the workspace.
///
/// Most recent first, with optional status, document, policy, and trigger-account
/// filters. Requires `ViewDetections`.
#[tracing::instrument(skip_all)]
async fn list_workspace_detections(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::ViewDetections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceDetectionsQuery>,
) -> Result<Json<WorkspaceDetectionsPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace detections");

    let workspace = &authz.workspace;
    let page = detections
        .list_for_workspace(workspace.id, pagination.into_cursor(), &query.into())
        .await?;

    Ok(Json(WorkspaceDetectionsPage::from_cursor_page(
        page,
        |row| WorkspaceDetectionSummary::from_row(row, WorkspaceRef::from(workspace)),
    )))
}

fn list_workspace_detections_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace detections")
        .description(
            "Returns all detections across the workspace, most recent first, \
             with optional status, document, policy, trigger-account, and input filename search filters. \
             Returns compact summaries and artifact indicators; fetch an individual detection \
             for full metadata and reviewer edits.",
        )
        .response::<200, Json<WorkspaceDetectionsPage>>()
}

/// Gets a specific detection.
#[tracing::instrument(
    skip_all,
    fields(
        detection_id = %path_params.detection_id,
    )
)]
async fn get_detection(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::ViewDetections>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
) -> Result<Json<WorkspaceDetection>> {
    tracing::debug!(target: TRACING_TARGET, "Getting detection");

    let workspace = &authz.workspace;
    let found = detections
        .find_with_documents(workspace.id, path_params.detection_id.as_uuid())
        .await?;
    tracing::debug!(target: TRACING_TARGET, "Detection retrieved");
    Ok(Json(detection_response(
        WorkspaceRef::from(workspace),
        found,
    )))
}

fn get_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get detection")
        .description("Returns the detection and its status for review.")
        .response::<200, Json<WorkspaceDetection>>()
}

/// Lists a workspace's detection trees, with pagination on detections.
#[tracing::instrument(skip_all)]
async fn list_detection_trees(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::ViewDetections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceDetectionsQuery>,
) -> Result<Json<DetectionTreesPage>> {
    let page = detections
        .list_trees(authz.workspace.id, pagination.into_cursor(), &query.into())
        .await?;
    Ok(Json(DetectionTreesPage::from_cursor_page(page, Into::into)))
}

fn list_detection_trees_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List detection trees")
        .description("Lists detection trees newest first with the same cursor pagination and status, document, policy, account, and filename search filters as the detection list. Each root contains its detection ID, live input metadata, intermediate availability, and all live redacted outputs newest first. Deleted input files are omitted; deleted or reclaimed outputs are omitted. Totals count detections, not child files. No file content is embedded.")
        .response::<200, Json<DetectionTreesPage>>()
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
        detection_id = %path_params.detection_id,
    )
)]
async fn stream_detection_events(
    State(detections): State<domain::WorkspaceDetectionService>,
    State(detection): State<DetectionQueue>,
    State(shutdown): State<CancellationToken>,
    authz: Authorized<markers::ViewDetections>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
) -> Result<SseResponse<DetectionStatusEvent>> {
    tracing::debug!(target: TRACING_TARGET, "Opening detection status stream");

    let workspace = &authz.workspace;
    let detection_id = path_params.detection_id.as_uuid();

    // Subscribe BEFORE reading the current status: core-NATS broadcasts are not
    // replayed, so a terminal status published between the read and the
    // subscription going live would otherwise be lost and the stream would hang.
    let mut updates = detection.subscribe_status(detection_id).await?;

    // Confirm the detection exists (and is workspace-scoped) so a bad id 404s here
    // rather than opening an empty stream.
    let detection_row = detections.find(workspace.id, detection_id).await?;
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

        while let Some(watched) = watch(&mut updates, STATUS_POLL_INTERVAL, &shutdown).await {
            let closed = matches!(watched, Watch::Closed);
            let status = match watched {
                // A live broadcast arrived; forward it (unless it moves backwards).
                Watch::Live(event) => event.status,
                Watch::Reread | Watch::Closed => {
                    match reread_detection_status(&detections, workspace_id, detection_id).await {
                        Some(status) => status,
                        None if closed => break,
                        None => continue,
                    }
                }
            };
            if status.phase() >= max_phase {
                max_phase = status.phase();
                yield status_event(&DetectionStatusEvent { detection_id, status });
            }
            // Stop once the detection settles, or the subscription ended.
            if !status.is_detecting() || closed {
                break;
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
        Ok(detection) => Some(detection.status),
        Err(err) => {
            tracing::debug!(target: TRACING_TARGET, error = %err, %detection_id, "Failed to re-read detection status");
            None
        }
    }
}

/// `OpenAPI` documentation for the detection status SSE stream.
fn stream_detection_events_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Stream detection status").description(
        "Opens a Server-Sent Events stream of the detection's status changes. \
             Emits the current status immediately, then each transition, and \
             ends once the detection settles (complete or failed). Each event's \
             `data` is a `DetectionStatusEvent` (see the response schema). \
             Authenticate with a Bearer token via a `fetch`-based client; the \
             native `EventSource` cannot send an `Authorization` header.",
    )
}

/// Builds a `status` SSE event carrying the detection's status change.
fn status_event(event: &DetectionStatusEvent) -> Event {
    sse_event("status", event)
}

/// Redacts a detection using its findings, storing the result.
///
/// Applies the detection's policies to its stored analysis, stores the
/// redacted bytes as a new document, and emits a redaction event. Requires
/// `RunRedactions` permission. A detection can be redacted more than once.
#[tracing::instrument(
    skip_all,
    fields(
        detection_id = %path_params.detection_id,
    )
)]
// reason: long but cohesive; splitting adds no clarity.
#[allow(clippy::too_many_lines)]
async fn redact_detection(
    State(redactions): State<domain::WorkspaceRedactionService>,
    authz: Authorized<markers::RunRedactions>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
    request: Option<ValidateJson<RedactWorkspaceDetection>>,
) -> Result<(StatusCode, Json<WorkspaceRedactionResult>)> {
    tracing::debug!(target: TRACING_TARGET, "Redacting detection");

    // The body is optional: without one there is no key.
    let key = request
        .and_then(|ValidateJson(request)| request.key)
        .map(RedactionKey::into_key_config);
    let redaction = redactions
        .redact(
            authz.origin(),
            &authz.workspace,
            path_params.detection_id.as_uuid(),
            key,
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceRedactionResult::from_model(
            &redaction,
            WorkspaceRef::from(&authz.workspace),
            authz.account.clone().into(),
        )),
    ))
}

fn redact_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Redact a detection")
        .description(
            "Applies the detection's policies to its stored analysis — with the \
             detection's accumulated reviewer edits layered on first (set via the edit endpoint) \
             — and produces a new redaction: a redacted document plus a review audit recording \
             what was redacted. A detection can be redacted more than once. The body is \
             optional; a policy using the `encrypt` or `hmac_hash` operator needs its `key` \
             (32 bytes, base64), which is used for this request only and never stored, so \
             only its holder can decrypt the values or match the pseudonyms. Requires \
             RunRedactions.",
        )
        .response::<201, Json<WorkspaceRedactionResult>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Sets (or clears) a detection's accumulated reviewer edits, which are layered over
/// its base analysis at redact time and persist between redactions. Requires
/// `RunRedactions`.
#[tracing::instrument(
    skip_all,
    fields(
        detection_id = %path_params.detection_id,
    )
)]
async fn edit_detection(
    State(detections): State<domain::WorkspaceDetectionService>,
    authz: Authorized<markers::RunRedactions>,
    Path(path_params): Path<WorkspaceDetectionPathParams>,
    ValidateJson(request): ValidateJson<EditWorkspaceDetection>,
) -> Result<Json<WorkspaceDetection>> {
    tracing::debug!(target: TRACING_TARGET, "Setting detection edits");

    let workspace = &authz.workspace;
    let detection_id = path_params.detection_id.as_uuid();

    // The domain validates the edits against the detection's base analysis before
    // storing (an unknown target or wrong modality → 400), so an invalid set never
    // wedges a later redaction.
    detections
        .set_edits(
            workspace.id,
            detection_id,
            authz.account_id,
            request.edit_set_rev,
            request.edits,
        )
        .await?;

    // Re-read so the response carries the same shape as the other detection
    // endpoints (trigger account and documents resolved).
    let found = detections
        .find_with_documents(workspace.id, detection_id)
        .await?;
    Ok(Json(detection_response(
        WorkspaceRef::from(workspace),
        found,
    )))
}

fn edit_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Edit a detection")
        .description(
            "Sets the detection's accumulated reviewer edits (suppress a false positive, retag a \
             detection, or add one the analysis missed), or clears them when `edits` is omitted. \
             The edits are validated against the detection's analysis before being stored (400 on \
             an unknown target or modality mismatch), persist on the detection, and are layered \
             over its base analysis at redact time so they survive between redactions. Pass the \
             `editSetRev` you last read; a stale value is rejected with 409. Requires \
             RunRedactions.",
        )
        .response::<200, Json<WorkspaceDetection>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all detection routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/detections",
            post_with(create_detection, create_detection_docs)
                .get_with(list_workspace_detections, list_workspace_detections_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}",
            get_with(get_detection, get_detection_docs)
                .patch_with(edit_detection, edit_detection_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/trees",
            get_with(list_detection_trees, list_detection_trees_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}/events",
            get_with(stream_detection_events, stream_detection_events_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/detections/{detectionId}/redactions",
            post_with(redact_detection, redact_detection_docs),
        )
        .with_path_items(|item| item.tag("Detections"))
}
