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

use elide_pipeline::governance::policy::Policy;
use elide_pipeline::provider::DocumentContext;
use nvisy_postgres::model::{
    NewWorkspaceDetection, NewWorkspaceDetectionJob, WorkspaceDetection as WorkspaceDetectionModel,
    WorkspacePipeline,
};
use nvisy_postgres::query::{
    DetectionCursor, DetectionDocuments, DetectionJobOutboxRepository, DetectionListRow,
    DetectionPolicyVersionRepository, PipelineReferenceRepository, WorkspaceDetectionRepository,
    WorkspaceDocumentRepository, WorkspacePipelineRepository, WorkspacePolicyRepository,
    WorkspacePolicyVersionRepository,
};
use nvisy_postgres::types::{CursorPage, CursorPagination, DetectionFilter, DetectionStatus, Json};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::input::{
    CreateAdhocDetectionInput, CreateDetectionInput, PipelineDefinitionInput,
};
use crate::domain::output::CreatedDetection;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{DetectionQueue, event};
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
    /// Pipeline the detection runs, if any.
    pipeline_id: Option<Uuid>,
    /// Effective document scope for the analysis job.
    scope: Option<DocumentContext>,
    /// Policies the analysis job resolves.
    policy_ids: Vec<Uuid>,
}

/// Creates and enqueues detections, and reads them back.
///
/// Holds the Postgres client (acquiring its own connection per call) and the
/// detection queue (to broadcast the initial status and wake the outbox drainer
/// after a create commits). Resolved per request from
/// [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspaceDetectionService {
    postgres: PgClient,
    detection: DetectionQueue,
}

impl WorkspaceDetectionService {
    /// Creates a [`WorkspaceDetectionService`] over its clients.
    #[must_use]
    pub fn new(postgres: PgClient, detection: DetectionQueue) -> Self {
        Self {
            postgres,
            detection,
        }
    }

    /// Starts a detection for a document through a pipeline: validates the request
    /// synchronously, snapshots the pipeline's retention override, and commits the
    /// detection row, its start event, its review thread, and its analysis job in
    /// one transaction, then broadcasts `Pending` and wakes the drainer.
    ///
    /// An idempotency key that matches an existing detection replays it instead of
    /// creating a new one.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the pipeline or the input document does not exist in the
    ///   workspace.
    /// - `Conflict` if the pipeline is not enabled.
    /// - `BadRequest` if the pipeline has no policies or its definition is invalid.
    /// - A database error if a connection or a query fails.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        pipeline_id: Uuid,
        idempotency_key: Option<String>,
        input: CreateDetectionInput,
    ) -> Result<CreatedDetection> {
        let workspace_id = origin.workspace_id;
        let mut conn = self.postgres.get_connection().await?;

        // Idempotent replay, checked before touching the pipeline: a repeated key
        // returns the detection created the first time, attributed to whoever
        // originally triggered it, even if the URL's pipeline has since been
        // deleted or disabled. The reported pipeline is the existing detection's
        // own — an idempotency key is workspace-scoped, so a replay may resolve a
        // detection created by a different pipeline (or none), and reporting this
        // URL's pipeline would misattribute it.
        if let Some(key) = &idempotency_key
            && let Some(existing) = conn
                .find_detection_by_idempotency_key(workspace_id, key)
                .await?
        {
            return self.replay(&mut conn, workspace_id, existing).await;
        }

        let pipeline = find_pipeline(&mut conn, workspace_id, pipeline_id).await?;

        // Only an enabled pipeline runs: a draft (still being configured) or a
        // disabled (paused) pipeline is rejected.
        if !pipeline.status.is_enabled() {
            return Err(ErrorKind::Conflict.with_message("Pipeline is not enabled"));
        }

        // Validate synchronously so a bad request fails fast (4xx) rather than as a
        // detection that immediately fails in the worker.
        let document = conn
            .find_document_in_workspace(workspace_id, input.document_id)
            .await?
            .ok_or_else(|| Error::not_found("document"))?;

        if conn.list_pipeline_policy_ids(pipeline.id).await?.is_empty() {
            return Err(ErrorKind::BadRequest
                .with_message("Pipeline has no policies; attach at least one before running"));
        }

        // Decode the pipeline definition now so an undecodable definition fails the
        // request synchronously (400) instead of failing later in the worker; the
        // worker rebuilds it from the same stored bytes.
        let _validated =
            PipelineDefinitionInput::from_parts(pipeline.definition.clone(), Vec::new()).map_err(
                |err| {
                    ErrorKind::BadRequest
                        .with_message("Pipeline definition is invalid")
                        .with_context(err.to_string())
                },
            )?;

        // Snapshot the pipeline's retention override onto the detection, so redaction
        // reproduces the retention the run used rather than the pipeline's current
        // value.
        let retention_override = pipeline
            .metadata
            .or_default()
            .retention
            .map(|over| Json::encode(&over));
        let new_detection = NewWorkspaceDetection {
            workspace_id,
            pipeline_id: Some(pipeline.id),
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
                    pipeline_id: Some(pipeline.id),
                    scope: input.scope,
                    policy_ids: Vec::new(),
                },
            )
            .await?;

        self.announce(&detection_row).await;

        tracing::info!(target: TRACING_TARGET, detection_id = %detection_row.id, "Detection queued");
        Ok(CreatedDetection {
            trigger_account_id: detection_row.account_id,
            detection: detection_row,
            documents: DetectionDocuments {
                input: Some(document.display_name),
            },
            created: true,
        })
    }

    /// Starts an ad-hoc detection against an explicit policy list, with no pipeline.
    ///
    /// Validates the document and every named policy synchronously, then commits and
    /// enqueues the detection the same way as [`create`]. An idempotency key replays
    /// an existing detection.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the input document or any named policy does not exist in the
    ///   workspace.
    /// - A database error if a connection or the create transaction fails.
    ///
    /// [`create`]: Self::create
    pub async fn create_adhoc(
        &self,
        origin: event::EventOrigin<'_>,
        idempotency_key: Option<String>,
        input: CreateAdhocDetectionInput,
    ) -> Result<CreatedDetection> {
        let workspace_id = origin.workspace_id;
        let mut conn = self.postgres.get_connection().await?;

        // Idempotent replay: a workspace-scoped key may resolve a detection created
        // by a pipeline, so the reported pipeline is the existing detection's own,
        // not unconditionally none.
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
            pipeline_id: None,
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
                    pipeline_id: None,
                    scope: input.scope,
                    policy_ids: input.policy_ids,
                },
            )
            .await?;

        self.announce(&detection_row).await;

        tracing::info!(target: TRACING_TARGET, detection_id = %detection_row.id, "Ad-hoc detection queued");
        Ok(CreatedDetection {
            trigger_account_id: origin.account_id,
            detection: detection_row,
            documents: DetectionDocuments {
                input: Some(document.display_name),
            },
            created: true,
        })
    }

    /// Builds the replay response for an existing detection matched by
    /// idempotency key. The owning pipeline is named by the detection's own
    /// `pipeline_id` (`None` for an ad-hoc detection).
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

    /// Lists a specific pipeline's detections with cursor pagination.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the pipeline does not exist in the workspace.
    /// - A database error if a connection or the query fails.
    pub async fn list_for_pipeline(
        &self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        pagination: CursorPagination<DetectionCursor>,
        filter: &DetectionFilter,
    ) -> Result<CursorPage<DetectionListRow>> {
        let mut conn = self.postgres.get_connection().await?;
        let pipeline = find_pipeline(&mut conn, workspace_id, pipeline_id).await?;
        Ok(conn
            .cursor_list_pipeline_detections(pipeline.id, pagination, filter)
            .await?)
    }

    /// Lists all of a workspace's detections with cursor pagination, including
    /// ad-hoc detections that name no pipeline.
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

    /// Finds a detection by id within a workspace, with the triggering account and
    /// its input document name — the context a single-detection response renders
    /// from. The owning pipeline is named by the detection's own `pipeline_id`.
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
        let (detection, _pipeline) = conn
            .find_workspace_detection_by_id(workspace_id, detection_id)
            .await?
            .ok_or_else(|| Error::not_found("detection"))?;
        let trigger_account_id = detection.account_id;
        let documents = conn
            .detection_document_names(workspace_id, &detection)
            .await?;
        Ok((detection, trigger_account_id, documents))
    }

    /// Finds a detection and its owning pipeline (if any) within a workspace, for a
    /// handler action (stream, redact) that then does its own work.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the detection does not exist in the workspace.
    /// - A database error if a connection or the query fails.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> Result<(WorkspaceDetectionModel, Option<WorkspacePipeline>)> {
        let mut conn = self.postgres.get_connection().await?;
        conn.find_workspace_detection_by_id(workspace_id, detection_id)
            .await?
            .ok_or_else(|| Error::not_found("detection"))
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
                    pipeline_id: detection.pipeline_id,
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

/// Finds a pipeline within a workspace by id or returns `NotFound`.
async fn find_pipeline(
    conn: &mut PgConn,
    workspace_id: Uuid,
    pipeline_id: Uuid,
) -> Result<WorkspacePipeline> {
    conn.find_pipeline_in_workspace_by_id(workspace_id, pipeline_id)
        .await?
        .map(|wc| wc.item)
        .ok_or_else(|| Error::not_found("pipeline"))
}
