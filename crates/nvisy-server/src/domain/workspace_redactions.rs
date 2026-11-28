//! Redaction domain service: redacting a detection, and reading redactions back.

use elide_pipeline::Audit;
use elide_pipeline::entity::EditSet;
use elide_pipeline::governance::policy::Policy;
use elide_pipeline::provider::KeyConfig;
use nvisy_postgres::model::{
    Blob, NewWorkspaceDocument, NewWorkspaceRedaction, Workspace,
    WorkspaceDetection as WorkspaceDetectionModel, WorkspaceDocument, WorkspaceRedaction,
};
use nvisy_postgres::query::{
    WorkspaceBlobRepository, WorkspaceDocumentRepository, WorkspaceRedactionRepository,
};
use nvisy_postgres::types::{
    CursorPage, CursorPagination, DocumentKind, RedactionFilter, TimeCursor, WithAccountRef,
};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::WorkspaceDetectionService;
use crate::response::{Error, ErrorKind, Resource, Result};
use crate::service::event::EventEmitter;
use crate::service::{ArtifactReader, ArtifactWriter, EngineService, StagedBlob, event};

/// Tracing target for redaction domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::redaction";

/// Redacts detections and reads redactions back.
#[derive(Clone)]
pub struct WorkspaceRedactionService {
    postgres: PgClient,
    detections: WorkspaceDetectionService,
    reader: ArtifactReader,
    writer: ArtifactWriter,
    engine: EngineService,
}

/// The pre-flight inputs a redaction reads under a connection before releasing it
/// for the slow analysis and staging work.
struct RedactInputs {
    detection: WorkspaceDetectionModel,
    /// The detection's input document.
    document: WorkspaceDocument,
    /// The input document's backing blob (for its extension and byte access).
    source_blob: Blob,
    /// The base analysis's backing blob, loaded to seed the working audit.
    audit_blob: Blob,
    policies: Vec<Policy>,
}

/// The objects a redaction staged before its commit.
struct Staged {
    /// The redacted document.
    output: StagedBlob,
    /// The redacted document's display name.
    output_name: String,
    /// The review audit.
    review: StagedBlob,
}

impl WorkspaceRedactionService {
    /// Creates a new [`WorkspaceRedactionService`].
    #[must_use]
    pub fn new(
        postgres: PgClient,
        detections: WorkspaceDetectionService,
        reader: ArtifactReader,
        writer: ArtifactWriter,
        engine: EngineService,
    ) -> Self {
        Self {
            postgres,
            detections,
            reader,
            writer,
            engine,
        }
    }

    /// Redacts a detection: applies its pinned policies to its stored analysis,
    /// with the detection's accumulated reviewer edits layered on first, and
    /// commits the redacted document, the review audit, and the redaction row
    /// together. A detection can be redacted more than once.
    ///
    /// Runs in three phases so a pooled connection is never held across the slow
    /// work: the pre-flight reads, then the analysis load, inference, and object
    /// staging with no connection held, then the commit. Every failure after
    /// staging reclaims the staged objects.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the detection does not exist in the workspace, or its
    ///   analysis has been reclaimed.
    /// - `Conflict` if the detection is not complete, or its document's content
    ///   is gone.
    /// - `BadRequest` if the stored edits do not fit the analysis, or a keyed
    ///   operator runs without a key.
    /// - An engine, storage, crypto, or database error otherwise.
    pub async fn redact(
        &self,
        origin: event::EventOrigin<'_>,
        workspace: &Workspace,
        detection_id: Uuid,
        key: Option<KeyConfig>,
    ) -> Result<WorkspaceRedaction> {
        let inputs = self.preflight(workspace.id, detection_id).await?;
        let staged = self.produce(origin, workspace, &inputs, key).await?;
        self.commit(origin, workspace, &inputs, staged).await
    }

    /// Redaction phase 1: resolves the detection, its pinned policies, its source
    /// document, and the blobs to read, releasing the connection before the slow
    /// work. Only the audit blob is resolved here; its bytes load in phase 2.
    async fn preflight(&self, workspace_id: Uuid, detection_id: Uuid) -> Result<RedactInputs> {
        let detection = self.detections.find(workspace_id, detection_id).await?;
        // A detection can only be redacted once its analysis is complete.
        if !detection.status.is_complete() {
            return Err(ErrorKind::Conflict.with_message("Detection is not ready to redact"));
        }

        // Redaction derives from this detection's base audit, so it re-redacts with
        // the exact policy versions the detection pinned when it ran, not the
        // policies' current versions. Resolved through the service (which manages its
        // own connection) before the pre-flight `conn` is acquired, so the two
        // connections never overlap and a burst of redacts cannot deadlock the pool.
        let policies = self
            .detections
            .resolve_pinned_policies(workspace_id, detection.id)
            .await?;

        let mut conn = self.postgres.get_connection().await?;

        // The source document is reachable only if it was not explicitly deleted;
        // surface a message that names the cause.
        let document = conn
            .find_document_in_workspace(workspace_id, detection.input_document_id)
            .await?
            .ok_or_else(Error::document_content_gone)?;
        let source_blob = conn
            .find_blob_by_id(document.blob_id.ok_or_else(Error::document_content_gone)?)
            .await?
            .ok_or_else(Error::document_content_gone)?;

        // The detection's base analysis blob, loaded to seed the working audit.
        // A completed detection with a NULL pointer had its bytes reclaimed (404);
        // an incomplete one has no analysis yet (409).
        let audit_blob_id = detection.audit_blob_id.ok_or_else(|| {
            if detection.status.is_complete() {
                ErrorKind::NotFound.with_message("The analysis for this detection has been deleted")
            } else {
                ErrorKind::Conflict.with_message("Detection has no analysis yet")
            }
        })?;
        let audit_blob = conn.find_blob_by_id(audit_blob_id).await?.ok_or_else(|| {
            ErrorKind::NotFound.with_message("The analysis for this detection has been deleted")
        })?;
        Ok(RedactInputs {
            detection,
            document,
            source_blob,
            audit_blob,
            policies,
        })
    }

    /// Redaction phase 2: loads the analysis, applies the reviewer edits, runs the
    /// redaction inference, and stages the produced objects, with no connection
    /// held.
    async fn produce(
        &self,
        origin: event::EventOrigin<'_>,
        workspace: &Workspace,
        inputs: &RedactInputs,
        key: Option<KeyConfig>,
    ) -> Result<Staged> {
        // The stored detection analysis is loaded into a working audit and never
        // mutated on disk: reviewer edits and the redaction outcome land on this
        // clone, which is persisted as the redaction's own review audit, leaving the
        // detection analysis immutable and re-redactable.
        let mut reviewed = self
            .reader
            .load_audit(&self.engine, workspace.id, &inputs.audit_blob)
            .await?;

        // Layer the detection's accumulated reviewer edits onto the working audit's
        // report before redaction. The edits are persisted on the detection (not passed
        // per request), so they survive between redactions. Both validation and landing
        // are report-relative (an unknown target, a self-contradiction, or an edit naming
        // a part of the wrong modality → 400, via `EditError`'s `From` impl), so a
        // reviewer is never told a decision took effect when the document says otherwise.
        // `apply` is fallible in its own right: some landing failures (a modality
        // mismatch) surface only when the edit lands. A malformed stored edit-set is a
        // 500, not a redaction against a silently dropped edit.
        if let Some(stored) = &inputs.detection.edit_set {
            let edits: EditSet =
                serde_json::from_value(stored.as_value().clone()).map_err(|err| {
                    ErrorKind::InternalServerError
                        .with_message("The detection's stored edits are malformed")
                        .with_context(err.to_string())
                })?;
            edits.validate(&reviewed.report)?;
            // The requester applies the stored edits, so the review records them as
            // the reviewer.
            edits.apply(&mut reviewed.report, &origin.account_id.to_string())?;
        }

        // The bytes can be purged between the blob lookup and this read (the
        // reaper released them as their window passed); that is the same "source
        // gone" outcome as a cleared pointer.
        let document = self
            .reader
            .build_document(&inputs.document, &inputs.source_blob, inputs.detection.id)
            .await
            .map_err(|err| match err.kind() {
                ErrorKind::NotFound => Error::document_content_gone(),
                _ => err,
            })?;

        // The keyed operators (HMAC, encrypt) run with the caller's key; a policy
        // using one without a key fails here as a 400. The codec params and document
        // context are read back from the audit, recorded at detect time.
        let redacted = self
            .engine
            .anonymize(document, &inputs.policies, &mut reviewed, key.as_ref())
            .await?;

        // Stage both produced objects (redacted document + review audit) outside the
        // transaction — object writes are not transactional — then commit their blobs,
        // document, audit, and redaction rows together. On rollback the staged objects
        // are reclaimed so no orphaned bytes accrue. Retention comes from the
        // detection's snapshotted override (the run's own retention), so re-redacting
        // reflects what the detection ran under.
        let retention = workspace.settings.or_default().retention;
        // The detection's snapshotted override, decoded to an owned value borrowed by
        // both staging calls below.
        let retention_override = inputs
            .detection
            .retention_override
            .as_ref()
            .map(nvisy_postgres::types::Json::or_default);
        let (staged_output, output_name) = self
            .writer
            .stage_redacted_document(
                &inputs.document,
                retention_override.as_ref(),
                &retention,
                redacted.bytes,
            )
            .await?;
        // Staging the review audit after the output means a failure here would strand
        // the already-written output object (no blob to reclaim it); discard it first.
        let staged_review = match self
            .writer
            .stage_audit(
                inputs.detection.workspace_id,
                retention_override.as_ref(),
                &retention,
                &reviewed,
            )
            .await
        {
            Ok(staged) => staged,
            Err(err) => {
                self.writer.discard_staged(&[&staged_output]).await;
                return Err(err);
            }
        };
        Ok(Staged {
            output: staged_output,
            output_name,
            review: staged_review,
        })
    }

    /// Redaction phase 3: commits the output document, the review audit blob, and
    /// the redaction row together, reclaiming the staged objects on failure and
    /// any staged object whose content deduplicated onto an existing blob.
    async fn commit(
        &self,
        origin: event::EventOrigin<'_>,
        workspace: &Workspace,
        inputs: &RedactInputs,
        staged: Staged,
    ) -> Result<WorkspaceRedaction> {
        // A failure to connect leaves both staged objects with no blob row for the
        // reaper to find, so discard them before returning, as every later
        // failure path does.
        let mut conn = match self.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                self.writer
                    .discard_staged(&[&staged.output, &staged.review])
                    .await;
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
                            account_id: origin.account_id,
                            blob_id: Uuid::nil(),
                            kind: Some(DocumentKind::Redacted),
                            display_name: Some(staged.output_name.clone()),
                            original_filename: Some(inputs.document.original_filename.clone()),
                            file_extension: Some(inputs.document.file_extension.clone()),
                            metadata: None,
                            expires_at: Some(staged.output.expires_at.into()),
                        },
                        staged.output.blob.clone(),
                    )
                    .await?;
                // The review analysis is a blob-ref on the redaction: resolve (share or
                // insert) its blob to record the reference, then point the redaction at
                // it via `review_audit_blob_id`.
                let review_resolved = conn.find_or_create_blob(staged.review.blob.clone()).await?;
                let redaction = conn
                    .create_redaction(NewWorkspaceRedaction {
                        detection_id: inputs.detection.id,
                        account_id: origin.account_id,
                        output_document_id: Some(output_document.id),
                        review_audit_blob_id: Some(review_resolved.id),
                        review_audit_expires_at: Some(staged.review.expires_at.into()),
                    })
                    .await?;
                conn.emit_event(
                    origin,
                    event::RedactionCreated {
                        detection_id: inputs.detection.id,
                        redaction_id: redaction.id,
                        input_document_name: Some(inputs.document.display_name.clone()),
                        notify: inputs.detection.account_id,
                    },
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
                self.writer
                    .discard_staged(&[&staged.output, &staged.review])
                    .await;
                return Err(err);
            }
        };

        // A committed blob whose content deduplicated onto an existing object leaves
        // the object staged for it orphaned (no row references it). Reclaim each
        // redundant staged object; best effort, a failure only defers it to a sweep.
        if output_path.as_deref() != Some(staged.output.blob.storage_path.as_str()) {
            self.writer.discard_staged(&[&staged.output]).await;
        }
        if review_path.as_deref() != Some(staged.review.blob.storage_path.as_str()) {
            self.writer.discard_staged(&[&staged.review]).await;
        }

        tracing::info!(
            target: TRACING_TARGET,
            detection_id = %inputs.detection.id,
            redaction_id = %redaction.id,
            "Detection redacted"
        );

        Ok(redaction)
    }

    /// Lists a detection's redactions, newest first.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the detection does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn list_for_detection(
        &self,
        workspace_id: Uuid,
        detection_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceRedaction>>> {
        let detection = self.detections.find(workspace_id, detection_id).await?;
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_detection_redactions(detection.id, pagination)
            .await?)
    }

    /// Lists a workspace's redactions, newest first.
    ///
    /// # Errors
    ///
    /// - A database error if the query fails.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
        filter: &RedactionFilter,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceRedaction>>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_redactions(workspace_id, pagination, filter)
            .await?)
    }

    /// Loads a redaction's review audit: the detection analysis with the
    /// reviewer's edits applied and the per-entity redaction outcome recorded.
    /// The connection is released before the object-store load.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the redaction does not exist in the workspace, or its
    ///   review audit has been reclaimed.
    /// - A storage, crypto, or engine error if loading the audit fails.
    pub async fn review_audit(&self, workspace_id: Uuid, redaction_id: Uuid) -> Result<Audit> {
        let review_blob = {
            let mut conn = self.postgres.get_connection().await?;
            let redaction = find_redaction(&mut conn, workspace_id, redaction_id).await?;
            self.reader
                .resolve_review_blob(&mut conn, &redaction)
                .await?
        };
        self.reader
            .load_audit(&self.engine, workspace_id, &review_blob)
            .await
    }
}

/// Loads a redaction scoped to the workspace, or a `NotFound`. A redaction id is
/// globally unique, so it is resolved within the workspace through its detection.
async fn find_redaction(
    conn: &mut PgConn,
    workspace_id: Uuid,
    redaction_id: Uuid,
) -> Result<WorkspaceRedaction> {
    conn.find_redaction_in_workspace(workspace_id, redaction_id)
        .await?
        .ok_or_else(|| Error::not_found(Resource::Redaction))
}
