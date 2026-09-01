//! Pipeline detection worker.
//!
//! Consumes [`DetectionJob`]s from the `DetectionStream` work-queue and runs a
//! detection's analysis in the background: builds the document, analyzes it with
//! the pipeline's policies, stores the encrypted audit, and marks the detection
//! `Complete` (or `Failed`). Each terminal transition is broadcast on the
//! detection's core-NATS status subject (for SSE watchers) and emitted as a
//! webhook event.

use std::time::Duration;

use elide_pipeline::RasterMode;
use nvisy_nats::stream::DetectionStream;
use nvisy_postgres::model::{UpdateWorkspaceDetection, WorkspaceDetection, WorkspacePipeline};
use nvisy_postgres::query::{
    EventOutboxRepository, WorkspaceDetectionRepository, WorkspaceFileRepository,
    WorkspaceRepository,
};
use nvisy_postgres::types::{DetectionStatus, Json, RasterPolicy, WorkspaceSettings};
use nvisy_postgres::{AsyncConnection, DieselError, PgError};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::job::DetectionJob;
use super::service::DetectionQueue;
use super::support::{
    FailDetection, FailOutcome, extract_detection_usage, fail_detection, resolve_policies,
};
use crate::extract::SecurityContext;
use crate::handler::request::PipelineDefinition;
use crate::handler::{ErrorKind, Result};
use crate::service::{
    DetectionRef, EngineService, EventOrigin, Infra, RunBlobStore, Worker, WorkspaceEvent,
    event_outbox_row,
};

/// Tracing target for detection worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::detection";

/// How long a detection claim stays valid before another delivery may re-claim
/// the detection. Set above `DetectionStream::ACK_WAIT` (15 min) so a
/// slow-but-healthy worker whose job is redelivered keeps its claim; only a
/// detection whose worker died (no progress past the lease) is re-claimed and
/// re-analyzed.
const DETECTION_LEASE: Duration = Duration::from_secs(30 * 60);

/// Background worker that runs pipeline detection off the request thread.
pub struct DetectionWorker {
    infra: Infra,
    engine: EngineService,
    blob: RunBlobStore,
    detection: DetectionQueue,
}

impl Worker for DetectionWorker {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "detection"
    }

    /// Runs the worker until cancelled, logging its lifecycle.
    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting detection worker");

        let result = self.run_inner(cancel).await;

        match &result {
            Ok(()) => tracing::info!(target: TRACING_TARGET, "Detection worker stopped"),
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Detection worker failed")
            }
        }

        result
    }
}

impl DetectionWorker {
    /// Creates a new [`DetectionWorker`].
    pub fn new(
        infra: Infra,
        engine: EngineService,
        blob: RunBlobStore,
        detection: DetectionQueue,
    ) -> Self {
        Self {
            infra,
            engine,
            blob,
            detection,
        }
    }

    /// Consumes detection jobs until cancelled.
    ///
    /// At-least-once with an explicit claim: a job is acked once it reaches a
    /// terminal outcome (complete, or marked failed), and nacked for redelivery
    /// on a transient error (a DB/pool blip before the detection could even be
    /// claimed), so a detection is never silently stranded in a non-terminal
    /// state. The claim (`claim_detection`) makes redelivery idempotent: a
    /// detection already being analyzed under a fresh lease is skipped.
    async fn run_inner(&self, cancel: CancellationToken) -> Result<()> {
        let subscriber = self
            .infra
            .nats
            .event_subscriber::<DetectionJob, DetectionStream>()
            .await?;
        let mut stream = subscriber.subscribe().await?;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(target: TRACING_TARGET, "Detection worker shutdown requested");
                    break;
                }
                result = stream.next_with_timeout(Duration::from_secs(5)) => {
                    match result {
                        Ok(Some(mut message)) => {
                            let job = message.payload().clone();
                            let outcome = self.run_job(job).await;
                            let ack_result = match outcome {
                                JobOutcome::Done => message.ack().await,
                                // Transient failure: redeliver instead of dropping
                                // the job, so the run is eventually settled.
                                JobOutcome::Retry => message.nack().await,
                            };
                            if let Err(err) = ack_result {
                                tracing::error!(target: TRACING_TARGET, error = %err, ?outcome, "Failed to ack/nack detection job");
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            tracing::error!(target: TRACING_TARGET, error = %err, "Error receiving detection job");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Runs one detection job: claims the detection, analyzes, and records the
    /// result.
    ///
    /// Returns [`JobOutcome::Retry`] when the job should be redelivered (a
    /// transient error before the detection was claimed), and [`JobOutcome::Done`]
    /// when it reached a terminal outcome or is safe to drop (missing detection,
    /// already claimed, already settled).
    #[tracing::instrument(skip_all, fields(detection_id = %job.detection_id, workspace_id = %job.workspace_id))]
    async fn run_job(&self, job: DetectionJob) -> JobOutcome {
        let mut conn = match self.infra.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                // No connection: the detection is still pending. Redeliver so it
                // is not stranded in a non-terminal state.
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to get connection for detection job");
                return JobOutcome::Retry;
            }
        };

        let (detection, pipeline) = match conn
            .find_workspace_detection_by_id(job.workspace_id, job.detection_id)
            .await
        {
            Ok(Some(pair)) => pair,
            Ok(None) => {
                tracing::warn!(target: TRACING_TARGET, "Detection job for a missing detection; dropping");
                return JobOutcome::Done;
            }
            Err(err) => {
                // Transient load error: redeliver rather than strand the detection.
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to load detection for detection job");
                return JobOutcome::Retry;
            }
        };

        // Nothing to do for a detection already past the pending/executing phase.
        if !detection.status.is_detecting() {
            tracing::debug!(target: TRACING_TARGET, status = %detection.status, "Detection is not detecting; dropping");
            return JobOutcome::Done;
        }

        // Atomically claim the detection (pending -> executing). A redelivery
        // whose claim is still fresh matches no row and is skipped, so a slow job
        // is never analyzed twice; only a detection whose worker died (stale
        // lease) is re-claimed.
        let stale_before = jiff::Timestamp::now() - DETECTION_LEASE;
        let claimed = match conn.claim_detection(detection.id, stale_before).await {
            Ok(Some(claimed)) => claimed,
            Ok(None) => {
                tracing::debug!(target: TRACING_TARGET, "Detection already claimed by another worker; skipping");
                return JobOutcome::Done;
            }
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to claim detection");
                return JobOutcome::Retry;
            }
        };
        // The claim stamped `claimed_at`; it fences the finalize (and a failure)
        // against a concurrent re-claim if this analysis outlives the lease.
        let Some(claim_token) = claimed.claimed_at else {
            tracing::error!(target: TRACING_TARGET, "Claimed detection has no claim timestamp; skipping");
            return JobOutcome::Done;
        };
        let claim_token: jiff::Timestamp = claim_token.into();

        self.detection
            .broadcast_status(detection.id, DetectionStatus::Executing)
            .await;

        // Release the connection before analysis: `detect` manages its own
        // connections across its phases, so holding this one across the (slow)
        // inference would pin a pooled connection per in-flight job and starve the
        // pool. It is re-acquired below only if the detection fails.
        drop(conn);

        if let Err(err) = self.detect(&job, &claimed, &pipeline, claim_token).await {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Detection failed");
            let mut conn = match self.infra.postgres.get_connection().await {
                Ok(conn) => conn,
                Err(err) => {
                    // No connection to persist the failure: the detection stays
                    // `Executing` with no queued job, so redeliver to drive it to a
                    // terminal state on a later attempt.
                    tracing::error!(target: TRACING_TARGET, error = %err, "Failed to get connection to fail detection");
                    return JobOutcome::Retry;
                }
            };
            let outcome = fail_detection(
                &mut conn,
                &self.detection,
                FailDetection {
                    workspace_id: job.workspace_id,
                    detection_id: detection.id,
                    pipeline_slug: pipeline.slug.clone(),
                    triggered_by: detection.account_id,
                    reason: &err.to_string(),
                    metadata: detection.metadata.or_default(),
                    claim: Some(claim_token),
                },
            )
            .await;
            // If the failure state could not be persisted, the detection is left
            // `Executing` with no queued job to reclaim its lease: redeliver so a
            // later attempt drives it to a terminal state rather than ack'ing a
            // detection that will hang.
            if outcome == FailOutcome::PersistFailed {
                return JobOutcome::Retry;
            }
        }
        JobOutcome::Done
    }

    /// Best-effort reclaim of a staged object whose file row did not commit. A
    /// failure only defers cleanup, so it is logged, never propagated.
    async fn discard_staged(&self, staged: &nvisy_postgres::model::NewWorkspaceFile) {
        if let Err(err) = self.blob.discard_staged_object(staged).await {
            tracing::warn!(
                target: TRACING_TARGET,
                error = %err,
                storage_path = %staged.storage_path,
                "Failed to reclaim orphaned staged object; left for a later sweep",
            );
        }
    }

    /// Stages a detection's enrichment intermediates, or returns `None` when the
    /// document produced none (a text/tabular modality: its artifact set is
    /// empty). Skipping the empty case avoids an intermediates file that a client
    /// would fetch only to find nothing.
    async fn stage_intermediates<T: serde::Serialize>(
        &self,
        pipeline: &WorkspacePipeline,
        settings: &WorkspaceSettings,
        account_id: Uuid,
        artifacts: &T,
    ) -> Result<Option<nvisy_postgres::model::NewWorkspaceFile>> {
        // The set serializes to `{ body, parts }`; an un-enriched document has a
        // null body and no parts, and there is nothing worth persisting.
        let value = serde_json::to_value(artifacts).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to serialize intermediates")
                .with_context(err.to_string())
        })?;
        let has_body = value.get("body").is_some_and(|body| !body.is_null());
        let has_parts = value
            .get("parts")
            .and_then(|parts| parts.as_object())
            .is_some_and(|parts| !parts.is_empty());
        if !has_body && !has_parts {
            return Ok(None);
        }

        let file = self
            .blob
            .stage_intermediates(pipeline, &settings.retention, account_id, artifacts)
            .await?;
        Ok(Some(file))
    }

    /// Performs the analysis and records the detection as `Complete`.
    ///
    /// Manages its own connection lifecycle in three phases so a pooled
    /// connection is never held across the analysis inference: phase 1 reads the
    /// inputs under a connection and releases it, phase 2 runs the (slow) document
    /// build, analysis, and audit staging with no connection held, and phase 3
    /// re-acquires a connection only for the finalize transaction.
    async fn detect(
        &self,
        job: &DetectionJob,
        detection: &WorkspaceDetection,
        pipeline: &WorkspacePipeline,
        claim_token: jiff::Timestamp,
    ) -> Result<()> {
        // Phase 1: read the inputs under a connection, then drop it.
        let (file, request, policies, settings) = {
            let mut conn = self.infra.postgres.get_connection().await?;

            let workspace = conn
                .find_workspace_by_id(job.workspace_id)
                .await?
                .ok_or_else(|| ErrorKind::NotFound.with_message("Workspace not found"))?;
            let file = conn
                .find_file_in_workspace(job.workspace_id, detection.input_file_id)
                .await?
                .ok_or_else(|| ErrorKind::NotFound.with_message("Input file not found"))?;

            let definition =
                PipelineDefinition::from_parts(pipeline.definition.clone(), Vec::new()).map_err(
                    |err| {
                        ErrorKind::InternalServerError
                            .with_message("Failed to decode pipeline definition")
                            .with_context(err.to_string())
                    },
                )?;

            // Parse the workspace settings once; both raster mode and retention
            // read it.
            let settings = workspace.settings.or_default();
            let request = self.engine.request_context(
                &definition,
                job.scope.clone(),
                raster_mode_of(&settings),
            );

            let policies =
                resolve_policies(&mut conn, &self.infra.crypto, job.workspace_id, pipeline.id)
                    .await?;
            if policies.is_empty() {
                return Err(ErrorKind::BadRequest
                    .with_message("Pipeline has no policies")
                    .with_resource("pipeline"));
            }

            (file, request, policies, settings)
        };

        // Phase 2: the slow work — document build, analysis inference, and audit
        // staging — runs with no DB connection held.
        let document = self.blob.build_document(&file, detection.id).await?;
        let analyzed = self.engine.analyze(document, &policies, &request).await?;
        let audit = &analyzed.audit;

        // Write the (non-transactional) audit object first, then commit its file
        // row together with the detection's usage and status in one transaction
        // below.
        let audit_file = self
            .blob
            .stage_analyzed_document(pipeline, &settings.retention, detection.account_id, audit)
            .await?;

        // Stage the enrichment intermediates (OCR layout, transcript) beside the
        // audit, so the client can read them and add entities the analysis missed.
        // A text/tabular document produces none — its artifacts serialize to an
        // empty set — so nothing is stored and the detection carries no
        // intermediates reference.
        let intermediates_file = self
            .stage_intermediates(
                pipeline,
                &settings,
                detection.account_id,
                &analyzed.artifacts,
            )
            .await?;

        // Record inference usage: per-model token rows into the usage table (the
        // usage aggregation surface) and the full per-recognizer report into
        // metadata for drill-down. Absent for a purely deterministic detection.
        // The report is layered onto the detection's existing metadata so tags and
        // any recorded error survive the write.
        let usage = extract_detection_usage(detection.id, audit);
        let metadata = usage.as_ref().map(|u| {
            let mut current = detection.metadata.or_default();
            current.usage = Some(u.report.clone());
            Json::encode(&current)
        });

        // Persist the audit file row, per-model usage, and the detection's
        // transition to `Complete` atomically: a partial failure would otherwise
        // strand usage rows or an audit pointer on a detection still marked
        // `Executing`. The finalize is fenced on our claim; if it went stale
        // (another worker re-claimed the detection past the lease), the whole
        // transaction rolls back so we do not stamp over the new owner's work or
        // leak usage/audit rows for a detection we lost. Kept to reclaim the
        // just-staged object if the transaction does not commit: on rollback its
        // `workspace_files` row never lands, so the row-driven reaper could never
        // find the object otherwise. Build the outbox row here so the finalize
        // transaction is `PgError`-typed for its rollback sentinel, and insert it
        // alongside the finalize so the `Complete` event commits atomically with
        // the detection.
        let completed_event = WorkspaceEvent::DetectionCompleted {
            detection: DetectionRef {
                detection_id: detection.id,
                pipeline_slug: pipeline.slug.clone(),
            },
            input_file_name: Some(file.display_name.clone()),
            notify: detection.account_id,
        };
        let outbox_row = event_outbox_row(
            EventOrigin {
                workspace_id: job.workspace_id,
                account_id: detection.account_id,
                security: &SecurityContext::default(),
            },
            &completed_event,
        )?;

        // Phase 3: re-acquire a connection only for the fenced finalize
        // transaction, so the pool was free during the analysis above.
        let mut conn = self.infra.postgres.get_connection().await?;
        // Kept to reclaim the staged objects if the transaction does not commit:
        // on rollback their `workspace_files` rows never land, so the row-driven
        // reaper could never find the objects otherwise.
        let staged_audit = audit_file.clone();
        let staged_intermediates = intermediates_file.clone();
        let finalized = conn
            .transaction(async |conn| {
                let audit_file_id = conn.create_workspace_file(audit_file).await?.id;
                let intermediates_file_id = match intermediates_file {
                    Some(file) => Some(conn.create_workspace_file(file).await?.id),
                    None => None,
                };
                if let Some(usage) = &usage {
                    conn.record_detection_usage(&usage.per_model).await?;
                }
                let finalized = conn
                    .finalize_detection(
                        detection.id,
                        claim_token,
                        UpdateWorkspaceDetection {
                            audit_file_id: Some(Some(audit_file_id)),
                            intermediates_file_id: Some(intermediates_file_id),
                            metadata,
                            ..Default::default()
                        },
                    )
                    .await?;
                if !finalized {
                    // Abort the file and usage inserts: the detection is no longer
                    // ours to finalize. `RollbackTransaction` unwinds the writes
                    // without being a real error; it is matched below.
                    return Err(PgError::Query(DieselError::RollbackTransaction));
                }
                conn.insert_event_outbox(outbox_row).await?;
                Ok::<_, PgError>(())
            })
            .await;

        match finalized {
            Ok(()) => {}
            Err(PgError::Query(DieselError::RollbackTransaction)) => {
                self.discard_staged(&staged_audit).await;
                if let Some(staged) = &staged_intermediates {
                    self.discard_staged(staged).await;
                }
                tracing::warn!(target: TRACING_TARGET, detection_id = %detection.id, "Claim went stale before finalize; another worker owns the detection");
                return Ok(());
            }
            Err(err) => {
                // The transaction rolled back, so the file rows never committed;
                // reclaim their objects before surfacing the failure.
                self.discard_staged(&staged_audit).await;
                if let Some(staged) = &staged_intermediates {
                    self.discard_staged(staged).await;
                }
                return Err(err.into());
            }
        }

        tracing::info!(target: TRACING_TARGET, detection_id = %detection.id, "Detection complete");
        self.detection
            .broadcast_status(detection.id, DetectionStatus::Complete)
            .await;

        Ok(())
    }
}

/// Whether a consumed detection job should be acked (done) or nacked (retry).
#[derive(Debug, Clone, Copy)]
enum JobOutcome {
    /// Reached a terminal outcome or is safe to drop; ack the message.
    Done,
    /// Transient error before the detection was claimed; nack for redelivery.
    Retry,
}

/// Maps a workspace's raster policy to the engine's per-detection
/// page-rasterisation mode.
fn raster_mode_of(settings: &WorkspaceSettings) -> RasterMode {
    match settings.raster {
        RasterPolicy::Auto => RasterMode::Auto,
        RasterPolicy::Always => RasterMode::always(),
        RasterPolicy::Never => RasterMode::Never,
    }
}
