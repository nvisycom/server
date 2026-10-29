//! Workspace detection request-side domain logic: create, enqueue, list, read.
//!
//! Owns the create/enqueue orchestration a detection handler would otherwise
//! inline — synchronous fail-fast validation, the retention-override snapshot, and
//! the transaction that creates the detection row, records the start event, and
//! enqueues the analysis job — plus the read queries. The async analysis and
//! redaction execution live in the detection
//! worker, not here; the service only owns the request side and meets the worker
//! at the outbox job and the [`DetectionQueue`]. The streaming (SSE) and
//! inference-and-staging (redact) actions stay in the handler, loading a detection
//! through [`find`].
//!
//! [`find`]: WorkspaceDetectionService::find

use elide_pipeline::entity::EditSet;
use elide_pipeline::governance::policy::Policy;
use elide_pipeline::provider::DocumentContext;
use nvisy_postgres::model::{
    NewWorkspaceDetection, NewWorkspaceDetectionJob, WorkspaceDetection as WorkspaceDetectionModel,
};
use nvisy_postgres::query::{
    DetectionCursor, DetectionDocuments, DetectionJobOutboxRepository, DetectionListRow,
    DetectionPolicyVersionRepository, EditWriteOutcome, WorkspaceDetectionRepository,
    WorkspaceDocumentRepository, WorkspacePolicyRepository, WorkspacePolicyVersionRepository,
};
use nvisy_postgres::types::{CursorPage, CursorPagination, DetectionFilter, DetectionStatus, Json};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::input::CreateDetectionInput;
use crate::domain::output::CreatedDetection;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{ArtifactReader, DetectionQueue, EngineService, event};
use crate::worker::detection::DetectionJob;

/// Tracing target for detection domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::detection";

/// The detection to commit, bundling the row and the analysis-job inputs that
/// [`commit_detection`] persists in one transaction.
///
/// [`commit_detection`]: WorkspaceDetectionService::commit_detection
struct CommitDetection {
    /// Workspace the detection belongs to.
    workspace_id: Uuid,
    /// The detection row to insert.
    new_detection: NewWorkspaceDetection,
    /// Effective document scope for the analysis job.
    scope: Option<DocumentContext>,
    /// Policies the analysis job resolves.
    policy_ids: Vec<Uuid>,
}

/// Creates and enqueues detections, and reads them back.
///
/// Holds the Postgres client (acquiring its own connection per call), the detection
/// queue (to broadcast the initial status and wake the outbox drainer after a create
/// commits), and the blob reader + engine (to load a detection's base analysis when
/// validating reviewer edits against it). Resolved per request from [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspaceDetectionService {
    postgres: PgClient,
    detection: DetectionQueue,
    reader: ArtifactReader,
    engine: EngineService,
}

impl WorkspaceDetectionService {
    /// Creates a [`WorkspaceDetectionService`] over its clients.
    #[must_use]
    pub fn new(
        postgres: PgClient,
        detection: DetectionQueue,
        reader: ArtifactReader,
        engine: EngineService,
    ) -> Self {
        Self {
            postgres,
            detection,
            reader,
            engine,
        }
    }

    /// Starts a detection for a document: validates the request synchronously,
    /// snapshots the retention override from the request, and commits the detection
    /// row, its start event, its review thread, and its analysis job in one
    /// transaction, then broadcasts `Pending` and wakes the drainer.
    ///
    /// Starts a detection against an explicit policy list over a document.
    ///
    /// Validates the document and every named policy synchronously, then commits and
    /// enqueues the detection. An idempotency key that matches an existing detection
    /// replays it instead of creating a new one.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the input document or any named policy does not exist in the
    ///   workspace.
    /// - A database error if a connection or the create transaction fails.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        idempotency_key: Option<String>,
        input: CreateDetectionInput,
    ) -> Result<CreatedDetection> {
        let workspace_id = origin.workspace_id;
        let mut conn = self.postgres.get_connection().await?;

        // Idempotent replay: a repeated workspace-scoped key returns the detection
        // created the first time, attributed to whoever originally triggered it.
        if let Some(key) = &idempotency_key
            && let Some(existing) = conn
                .find_detection_by_idempotency_key(workspace_id, key)
                .await?
        {
            return self.replay(&mut conn, workspace_id, existing).await;
        }

        let document = conn
            .find_document_in_workspace(workspace_id, input.document_id)
            .await?
            .ok_or_else(|| Error::not_found("document"))?;

        // Every named policy must resolve to a live policy (any kind) in the
        // workspace, so an unknown id is a 404 now rather than a worker failure.
        for policy_id in &input.policy_ids {
            if conn
                .find_policy_in_workspace_by_id(workspace_id, *policy_id)
                .await?
                .is_none()
            {
                return Err(Error::not_found("policy"));
            }
        }

        let retention_override = input.retention_override.map(|over| Json::encode(&over));
        let new_detection = NewWorkspaceDetection {
            workspace_id,
            input_document_id: document.id,
            account_id: origin.account_id,
            status: Some(DetectionStatus::Pending),
            idempotency_key,
            retention_override,
            ..Default::default()
        };

        let detection_row = self
            .commit_detection(
                &mut conn,
                origin,
                CommitDetection {
                    workspace_id,
                    new_detection,
                    scope: input.scope,
                    policy_ids: input.policy_ids,
                },
            )
            .await?;

        self.announce(&detection_row).await;

        tracing::info!(target: TRACING_TARGET, detection_id = %detection_row.id, "Detection queued");
        Ok(CreatedDetection {
            trigger_account_id: origin.account_id,
            detection: detection_row,
            documents: DetectionDocuments {
                input: Some(document.display_name),
            },
            created: true,
        })
    }

    /// Builds the replay response for an existing detection matched by idempotency
    /// key.
    async fn replay(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        existing: WorkspaceDetectionModel,
    ) -> Result<CreatedDetection> {
        let documents = conn
            .detection_document_names(workspace_id, &existing)
            .await?;
        Ok(CreatedDetection {
            trigger_account_id: existing.account_id,
            detection: existing,
            documents,
            created: false,
        })
    }

    /// Lists all of a workspace's detections with cursor pagination.
    ///
    /// # Errors
    ///
    /// - A database error if a connection or the query fails.
    pub async fn list_for_workspace(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<DetectionCursor>,
        filter: &DetectionFilter,
    ) -> Result<CursorPage<DetectionListRow>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_detections(workspace_id, pagination, filter)
            .await?)
    }

    /// Lists a workspace's detection trees using the detection filters
    /// and cursor order. Children are fetched in one batch after paginating roots.
    ///
    /// # Errors
    ///
    /// - A database error if a connection or query fails.
    pub async fn list_trees(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<DetectionCursor>,
        filter: &DetectionFilter,
    ) -> Result<CursorPage<nvisy_postgres::query::DetectionTreeRow>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_detection_trees(workspace_id, pagination, filter)
            .await?)
    }

    /// Finds a detection by id within a workspace, with the triggering account and
    /// its input document name — the context a single-detection response renders from.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the detection does not exist in the workspace.
    /// - A database error if a connection or a query fails.
    pub async fn get(
        &self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> Result<(WorkspaceDetectionModel, Uuid, DetectionDocuments)> {
        let mut conn = self.postgres.get_connection().await?;
        let detection = conn
            .find_workspace_detection_by_id(workspace_id, detection_id)
            .await?
            .ok_or_else(|| Error::not_found("detection"))?;
        let trigger_account_id = detection.account_id;
        let documents = conn
            .detection_document_names(workspace_id, &detection)
            .await?;
        Ok((detection, trigger_account_id, documents))
    }

    /// Finds a detection within a workspace, for a handler action (stream, redact)
    /// that then does its own work.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the detection does not exist in the workspace.
    /// - A database error if a connection or the query fails.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> Result<WorkspaceDetectionModel> {
        let mut conn = self.postgres.get_connection().await?;
        conn.find_workspace_detection_by_id(workspace_id, detection_id)
            .await?
            .ok_or_else(|| Error::not_found("detection"))
    }

    /// Persists a detection's accumulated reviewer edits (`edit_set`) under an
    /// optimistic-concurrency guard, the manual corrections layered over the base
    /// analysis at redact time. Passing `None` clears them. `expected_rev` is the
    /// `edit_set_rev` the caller read; the write lands only if it still matches. The
    /// edits live on the detection so they survive between redactions and a solo user
    /// keeps their work without opening a review.
    ///
    /// The edits are validated against the detection's base analysis before being
    /// stored — an edit naming an unknown target or the wrong modality is rejected
    /// here (`BadRequest`), so an invalid set is never persisted to wedge a later
    /// redaction. Clearing (no edits) needs no analysis and skips this.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the detection does not exist in the workspace, or its analysis
    ///   has been reclaimed.
    /// - `Conflict` if the detection has no analysis yet, or `expected_rev` is stale
    ///   (a concurrent edit won).
    /// - `BadRequest` if the edits do not validate against the base analysis.
    /// - A database error if a connection or the query fails.
    pub async fn set_edits(
        &self,
        workspace_id: Uuid,
        detection_id: Uuid,
        expected_rev: i32,
        edits: Option<EditSet>,
    ) -> Result<WorkspaceDetectionModel> {
        // Validate the edits against the detection's base analysis before storing. An
        // unknown target, a self-contradiction, or an edit naming the wrong modality
        // is rejected now rather than silently stored to fail a later redaction. This
        // is the same report-relative check the redact path applies.
        if let Some(edits) = &edits {
            let detection = self.find(workspace_id, detection_id).await?;
            let mut audit = self.load_base_audit(workspace_id, &detection).await?;
            edits.validate(&audit.report)?;
            // Trial-apply on the loaded audit to surface landing failures validation
            // alone does not (e.g. a modality mismatch); the audit is then discarded.
            edits.apply(&mut audit.report)?;
        }

        let mut conn = self.postgres.get_connection().await?;
        // The postgres layer stores the edit-set opaquely as raw JSON (it has no
        // dependency on the engine's `EditSet` type), so encode to a value here.
        let edit_set = edits
            .as_ref()
            .map(|edits| {
                serde_json::to_value(edits)
                    .map(Json::from_raw)
                    .map_err(|err| {
                        ErrorKind::InternalServerError
                            .with_message("Failed to encode detection edits")
                            .with_context(err.to_string())
                    })
            })
            .transpose()?;
        match conn
            .set_detection_edits(workspace_id, detection_id, expected_rev, edit_set)
            .await?
        {
            EditWriteOutcome::Updated(detection) => Ok(*detection),
            EditWriteOutcome::Stale => Err(ErrorKind::Conflict.with_message(
                "The detection's edits changed since you loaded them; reload and retry",
            )),
            EditWriteOutcome::Absent => Err(Error::not_found("detection")),
        }
    }

    /// Loads a detection's base analysis (its [`Audit`]), for validating reviewer
    /// edits against its report. Resolves the audit blob under a short-lived
    /// connection, then decodes it with the engine (no connection held across the
    /// object-store round-trip).
    ///
    /// [`Audit`]: elide_pipeline::Audit
    ///
    /// # Errors
    ///
    /// - `Conflict` if the detection has no analysis yet.
    /// - `NotFound` if the analysis blob has been reclaimed.
    /// - `InternalServerError` if the stored audit cannot be decoded.
    /// - A database error if a lookup fails.
    async fn load_base_audit(
        &self,
        workspace_id: Uuid,
        detection: &WorkspaceDetectionModel,
    ) -> Result<elide_pipeline::Audit> {
        let audit_blob = {
            let mut conn = self.postgres.get_connection().await?;
            self.reader.resolve_audit_blob(&mut conn, detection).await?
        };
        self.reader
            .load_audit(self.engine.engine(), workspace_id, &audit_blob)
            .await
    }

    /// Loads the exact policy definitions a detection was analyzed with, from the
    /// versions pinned when it ran.
    ///
    /// Redaction derives from a detection's base audit, so it must reproduce the
    /// policy versions that produced that audit rather than the policies' current
    /// versions — a policy edited since the detection ran would otherwise redact
    /// against a definition inconsistent with the analysis. A pinned version
    /// resolves even after its policy is soft-deleted, so a historical detection can
    /// always be re-redacted.
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if a pinned policy version can no longer be loaded,
    ///   or if a stored policy definition is malformed.
    /// - A database error if a connection or a query fails.
    pub async fn resolve_pinned_policies(
        &self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> Result<Vec<Policy>> {
        let mut conn = self.postgres.get_connection().await?;
        let version_ids = conn.list_detection_policy_versions(detection_id).await?;
        let mut policies = Vec::with_capacity(version_ids.len());
        for version_id in version_ids {
            let version = conn
                .find_policy_version(workspace_id, version_id)
                .await?
                .ok_or_else(|| {
                    ErrorKind::InternalServerError
                        .with_message("A pinned policy version is no longer available")
                        .with_context(format!("policy_version_id: {version_id}"))
                })?;
            policies.push(parse_definition(version.id, version.definition)?);
        }
        Ok(policies)
    }

    /// Commits the detection row, its start event, and its analysis job in one
    /// transaction.
    ///
    /// The job goes onto the outbox rather than being published inline, so the
    /// detection is never lost to a publish that failed after the row committed, nor
    /// marked failed for a publish that in fact went through: the drainer relays the
    /// outbox row to the work-queue, and the worker's claim dedups a redelivery.
    async fn commit_detection(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        detection: CommitDetection,
    ) -> Result<WorkspaceDetectionModel> {
        conn.transaction(async |conn| {
            let detection_row = conn
                .create_workspace_detection(detection.new_detection)
                .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::DetectionStarted(event::DetectionStarted {
                    detection_id: detection_row.id,
                }),
            )
            .await?;

            // Detection is just analysis: it produces findings and creates no
            // review. A reviewer opens a review explicitly (for a purpose) and links
            // the relevant detections/redactions to it.
            let job = DetectionJob {
                workspace_id: detection.workspace_id,
                detection_id: detection_row.id,
                scope: detection.scope,
                policy_ids: detection.policy_ids,
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
        .await
    }

    /// Broadcasts `Pending` and wakes the outbox drainer after a detection commits.
    ///
    /// Broadcasting before waking avoids racing this instance's own worker: waking
    /// first could let the worker publish `Executing` ahead of this `Pending` and
    /// invert the order a watcher sees. Best-effort — the detection row is
    /// authoritative and the stream still guards against a late `Pending`.
    async fn announce(&self, detection: &WorkspaceDetectionModel) {
        self.detection
            .broadcast_status(detection.id, DetectionStatus::Pending)
            .await;
        self.detection.wake_drainer();
    }
}

/// Deserializes a stored policy definition (plaintext JSONB) into an engine
/// [`Policy`], failing if the stored body is malformed.
fn parse_definition(version_id: Uuid, definition: serde_json::Value) -> Result<Policy> {
    serde_json::from_value(definition).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Stored policy definition is malformed")
            .with_context(format!("policy_version_id: {version_id}: {err}"))
    })
}
