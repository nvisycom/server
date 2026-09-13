//! Workspace detection request-side domain logic: create, enqueue, list, read.
//!
//! Owns the create/enqueue orchestration a detection handler would otherwise
//! inline — synchronous fail-fast validation, the retention-override snapshot, and
//! the transaction that creates the detection row, records the start event, ensures
//! the document's review thread, and enqueues the analysis job — plus the read
//! queries. The async analysis and redaction execution live in the detection
//! worker, not here; the service only owns the request side and meets the worker
//! at the outbox job and the [`DetectionQueue`]. The streaming (SSE) and
//! inference-and-staging (redact) actions stay in the handler, loading a detection
//! through [`find`](WorkspaceDetectionService::find).

use elide_pipeline::provider::DocumentContext;
use nvisy_postgres::model::{
    NewWorkspaceDetection, NewWorkspaceDetectionJob, WorkspaceDetection as WorkspaceDetectionModel,
    WorkspacePipeline,
};
use nvisy_postgres::query::{
    DetectionCursor, DetectionDocuments, DetectionJobOutboxRepository, DetectionListRow,
    PipelineReferenceRepository, WorkspaceDetectionRepository, WorkspaceDocumentRepository,
    WorkspacePipelineRepository, WorkspacePolicyRepository, WorkspaceThreadRepository,
};
use nvisy_postgres::types::{
    CursorPage, CursorPagination, DetectionFilter, DetectionStatus, Handle, Json,
};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::input::{
    CreateAdhocDetectionInput, CreateDetectionInput, PipelineDefinitionInput,
};
use crate::domain::output::CreatedDetection;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{DetectionJob, DetectionQueue, event};

/// Tracing target for detection domain operations.
const TRACING_TARGET: &str = "nvisy_server::service::detection";

/// Creates and enqueues detections, and reads them back.
///
/// Holds the Postgres client (acquiring its own connection per call) and the
/// detection queue (to broadcast the initial status and wake the outbox drainer
/// after a create commits). Resolved per request from
/// [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct WorkspaceDetectionService {
    postgres: PgClient,
    detection: DetectionQueue,
}

impl WorkspaceDetectionService {
    /// Creates a [`WorkspaceDetectionService`] over its clients.
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
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        pipeline_slug: &str,
        idempotency_key: Option<String>,
        input: CreateDetectionInput,
    ) -> Result<CreatedDetection> {
        let workspace_id = origin.workspace_id;
        let mut conn = self.postgres.get_connection().await?;

        let pipeline = find_pipeline(&mut conn, workspace_id, pipeline_slug).await?;

        // Only an enabled pipeline runs: a draft (still being configured) or a
        // disabled (paused) pipeline is rejected.
        if !pipeline.status.is_enabled() {
            return Err(ErrorKind::Conflict
                .with_message("Pipeline is not enabled")
                .with_resource("pipeline"));
        }

        // Idempotent replay: a repeated key returns the detection created the first
        // time, attributed to whoever originally triggered it.
        if let Some(key) = &idempotency_key
            && let Some(existing) = conn
                .find_detection_by_idempotency_key(workspace_id, key)
                .await?
        {
            let documents = conn
                .detection_document_names(workspace_id, &existing)
                .await?;
            return Ok(CreatedDetection {
                trigger_account_id: existing.account_id,
                detection: existing,
                pipeline_slug: Some(pipeline.slug),
                documents,
                created: false,
            });
        }

        // Validate synchronously so a bad request fails fast (4xx) rather than as a
        // detection that immediately fails in the worker.
        let document = conn
            .find_document_in_workspace(workspace_id, input.document_id)
            .await?
            .ok_or_else(|| Error::not_found("document"))?;

        if conn.list_pipeline_policy_ids(pipeline.id).await?.is_empty() {
            return Err(ErrorKind::BadRequest
                .with_message("Pipeline has no policies; attach at least one before running")
                .with_resource("pipeline"));
        }

        // Decode the pipeline definition now so an undecodable definition fails the
        // request synchronously (400) instead of failing later in the worker; the
        // worker rebuilds it from the same stored bytes.
        let _validated =
            PipelineDefinitionInput::from_parts(pipeline.definition.clone(), Vec::new()).map_err(
                |err| {
                    ErrorKind::BadRequest
                        .with_message("Pipeline definition is invalid")
                        .with_resource("pipeline")
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
                workspace_id,
                document.id,
                new_detection,
                Some(pipeline.slug.clone()),
                input.scope,
                Vec::new(),
            )
            .await?;

        self.announce(&detection_row).await;

        tracing::info!(target: TRACING_TARGET, detection_id = %detection_row.id, "Detection queued");
        Ok(CreatedDetection {
            trigger_account_id: detection_row.account_id,
            detection: detection_row,
            pipeline_slug: Some(pipeline.slug),
            documents: DetectionDocuments {
                input: Some(document.display_name),
            },
            created: true,
        })
    }

    /// Starts an ad-hoc detection against an explicit policy list, with no pipeline.
    ///
    /// Validates the document and every named policy synchronously, then commits and
    /// enqueues the detection the same way as [`create`](Self::create). An
    /// idempotency key replays an existing detection.
    pub async fn create_adhoc(
        &self,
        origin: event::EventOrigin<'_>,
        idempotency_key: Option<String>,
        input: CreateAdhocDetectionInput,
    ) -> Result<CreatedDetection> {
        let workspace_id = origin.workspace_id;
        let mut conn = self.postgres.get_connection().await?;

        if let Some(key) = &idempotency_key
            && let Some(existing) = conn
                .find_detection_by_idempotency_key(workspace_id, key)
                .await?
        {
            let documents = conn
                .detection_document_names(workspace_id, &existing)
                .await?;
            return Ok(CreatedDetection {
                trigger_account_id: existing.account_id,
                detection: existing,
                pipeline_slug: None,
                documents,
                created: false,
            });
        }

        let document = conn
            .find_document_in_workspace(workspace_id, input.document_id)
            .await?
            .ok_or_else(|| Error::not_found("document"))?;

        // Every named policy must resolve to a live policy (any kind) in the
        // workspace, so an unknown slug is a 404 now rather than a worker failure.
        for slug in &input.policy_slugs {
            if conn
                .find_policy_in_workspace_by_slug(workspace_id, slug.as_str())
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
                workspace_id,
                document.id,
                new_detection,
                None,
                input.scope,
                input.policy_slugs,
            )
            .await?;

        self.announce(&detection_row).await;

        tracing::info!(target: TRACING_TARGET, detection_id = %detection_row.id, "Ad-hoc detection queued");
        Ok(CreatedDetection {
            trigger_account_id: origin.account_id,
            detection: detection_row,
            pipeline_slug: None,
            documents: DetectionDocuments {
                input: Some(document.display_name),
            },
            created: true,
        })
    }

    /// Lists a specific pipeline's detections with cursor pagination.
    pub async fn list_for_pipeline(
        &self,
        workspace_id: Uuid,
        pipeline_slug: &str,
        pagination: CursorPagination<DetectionCursor>,
        filter: &DetectionFilter,
    ) -> Result<CursorPage<DetectionListRow>> {
        let mut conn = self.postgres.get_connection().await?;
        let pipeline = find_pipeline(&mut conn, workspace_id, pipeline_slug).await?;
        Ok(conn
            .cursor_list_pipeline_detections(pipeline.id, pagination, filter)
            .await?)
    }

    /// Lists all of a workspace's detections with cursor pagination, including
    /// ad-hoc detections that name no pipeline.
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

    /// Finds a detection by id within a workspace, with its owning pipeline (if
    /// any), the triggering account, and its input document name — the context a
    /// single-detection response renders from.
    pub async fn get(
        &self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> Result<(
        WorkspaceDetectionModel,
        Option<Handle>,
        Uuid,
        DetectionDocuments,
    )> {
        let mut conn = self.postgres.get_connection().await?;
        let (detection, pipeline) = conn
            .find_workspace_detection_by_id(workspace_id, detection_id)
            .await?
            .ok_or_else(|| Error::not_found("detection"))?;
        let trigger_account_id = detection.account_id;
        let documents = conn
            .detection_document_names(workspace_id, &detection)
            .await?;
        Ok((
            detection,
            pipeline.map(|p| p.slug),
            trigger_account_id,
            documents,
        ))
    }

    /// Finds a detection and its owning pipeline (if any) within a workspace, for a
    /// handler action (stream, redact) that then does its own work.
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

    /// Commits the detection row, its start event, its review thread, and its
    /// analysis job in one transaction.
    ///
    /// The job goes onto the outbox rather than being published inline, so the
    /// detection is never lost to a publish that failed after the row committed, nor
    /// marked failed for a publish that in fact went through: the drainer relays the
    /// outbox row to the work-queue, and the worker's claim dedups a redelivery.
    async fn commit_detection(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        workspace_id: Uuid,
        document_id: Uuid,
        new_detection: NewWorkspaceDetection,
        pipeline_slug: Option<Handle>,
        scope: Option<DocumentContext>,
        policy_slugs: Vec<Handle>,
    ) -> Result<WorkspaceDetectionModel> {
        let account_id = origin.account_id;
        conn.transaction(async |conn| {
            let detection_row = conn.create_workspace_detection(new_detection).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::DetectionStarted(event::DetectionStarted {
                    detection_id: detection_row.id,
                    pipeline_slug,
                }),
            )
            .await?;

            // The document's review is its thread: ensure it exists (one live thread
            // per document) so this detection has somewhere to be reviewed, and
            // reopen it if a prior review had resolved.
            let thread = conn
                .find_or_create_document_thread(workspace_id, document_id, account_id)
                .await?;
            conn.reopen_review(thread.id, account_id).await?;

            let job = DetectionJob {
                workspace_id,
                detection_id: detection_row.id,
                scope,
                policy_slugs,
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
