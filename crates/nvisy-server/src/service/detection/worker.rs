//! Pipeline detection worker.
//!
//! Consumes [`DetectionJob`]s from the `DetectionStream` work-queue and runs a
//! run's detection (analyze) in the background: builds the document, analyzes it
//! with the pipeline's policies, stores the encrypted audit, and marks the run
//! `Analyzed` (or `Failed`). Each terminal transition is broadcast on the run's
//! core-NATS status subject (for SSE watchers) and emitted as a webhook event.

use std::time::Duration;

use elide_pipeline::RasterMode;
use nvisy_nats::stream::DetectionStream;
use nvisy_postgres::model::{UpdateWorkspacePipelineRun, WorkspacePipeline, WorkspacePipelineRun};
use nvisy_postgres::query::{
    EventOutboxRepository, WorkspaceFileRepository, WorkspacePipelineRunRepository,
    WorkspaceRepository,
};
use nvisy_postgres::types::{Json, OcrPolicy, PipelineRunStatus, WorkspaceSettings};
use nvisy_postgres::{AsyncConnection, DieselError, PgConn, PgError};
use tokio_util::sync::CancellationToken;

use super::job::DetectionJob;
use super::service::DetectionQueue;
use super::support::{FailRun, extract_run_usage, fail_run, resolve_policies};
use crate::extract::SecurityContext;
use crate::handler::request::PipelineDefinition;
use crate::handler::{ErrorKind, Result};
use crate::service::{
    EngineService, EventOrigin, Infra, PipelineRunRef, RunBlobStore, Worker, WorkspaceEvent,
    event_outbox_row,
};

/// Tracing target for detection worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::detection";

/// How long a detection claim stays valid before another delivery may re-claim
/// the run. Set above `DetectionStream::ACK_WAIT` (15 min) so a slow-but-healthy
/// worker whose job is redelivered keeps its claim; only a run whose worker died
/// (no progress past the lease) is re-claimed and re-analyzed.
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
    /// terminal outcome (analyzed, or marked failed), and nacked for redelivery
    /// on a transient error (a DB/pool blip before the run could even be
    /// claimed), so a run is never silently stranded in a non-terminal state.
    /// The claim (`claim_run_for_detection`) makes redelivery idempotent: a run
    /// already being analyzed under a fresh lease is skipped.
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

    /// Runs one detection job: claims the run, analyzes, and records the result.
    ///
    /// Returns [`JobOutcome::Retry`] when the job should be redelivered (a
    /// transient error before the run was claimed), and [`JobOutcome::Done`]
    /// when it reached a terminal outcome or is safe to drop (missing run,
    /// already claimed, already settled).
    #[tracing::instrument(skip_all, fields(run_id = %job.run_id, workspace_id = %job.workspace_id))]
    async fn run_job(&self, job: DetectionJob) -> JobOutcome {
        let mut conn = match self.infra.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                // No connection: the run is still queued. Redeliver so it is not
                // stranded in a non-terminal state.
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to get connection for detection job");
                return JobOutcome::Retry;
            }
        };

        let (run, pipeline) = match conn
            .find_workspace_run_by_id(job.workspace_id, job.run_id)
            .await
        {
            Ok(Some(pair)) => pair,
            Ok(None) => {
                tracing::warn!(target: TRACING_TARGET, "Detection job for a missing run; dropping");
                return JobOutcome::Done;
            }
            Err(err) => {
                // Transient load error: redeliver rather than strand the run.
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to load run for detection job");
                return JobOutcome::Retry;
            }
        };

        // Nothing to do for a run already past the queued/analyzing phase.
        if !run.status.is_detecting() {
            tracing::debug!(target: TRACING_TARGET, status = %run.status, "Run is not detecting; dropping");
            return JobOutcome::Done;
        }

        // Atomically claim the run (queued -> analyzing). A redelivery whose
        // claim is still fresh matches no row and is skipped, so a slow job is
        // never analyzed twice; only a run whose worker died (stale lease) is
        // re-claimed.
        let stale_before = jiff::Timestamp::now() - DETECTION_LEASE;
        let claimed = match conn.claim_run_for_detection(run.id, stale_before).await {
            Ok(Some(claimed)) => claimed,
            Ok(None) => {
                tracing::debug!(target: TRACING_TARGET, "Run already claimed by another worker; skipping");
                return JobOutcome::Done;
            }
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to claim run for detection");
                return JobOutcome::Retry;
            }
        };
        // The claim stamped `claimed_at`; it fences the finalize (and a failure)
        // against a concurrent re-claim if this analysis outlives the lease.
        let Some(claim_token) = claimed.claimed_at else {
            tracing::error!(target: TRACING_TARGET, "Claimed run has no claim timestamp; skipping");
            return JobOutcome::Done;
        };
        let claim_token: jiff::Timestamp = claim_token.into();

        self.detection
            .broadcast_status(run.id, PipelineRunStatus::Analyzing)
            .await;

        if let Err(err) = self
            .detect(&mut conn, &job, &claimed, &pipeline, claim_token)
            .await
        {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Detection failed");
            fail_run(
                &mut conn,
                &self.detection,
                FailRun {
                    workspace_id: job.workspace_id,
                    run_id: run.id,
                    pipeline_slug: pipeline.slug.clone(),
                    triggered_by: run.account_id,
                    reason: &err.to_string(),
                    claim: Some(claim_token),
                },
            )
            .await;
        }
        JobOutcome::Done
    }

    /// Best-effort reclaim of a staged audit object whose file row did not
    /// commit. A failure only defers cleanup, so it is logged, never propagated.
    async fn discard_staged_audit(&self, staged: &nvisy_postgres::model::NewWorkspaceFile) {
        if let Err(err) = self.blob.discard_staged_audit(staged).await {
            tracing::warn!(
                target: TRACING_TARGET,
                error = %err,
                storage_path = %staged.storage_path,
                "Failed to reclaim orphaned audit object; left for a later sweep",
            );
        }
    }

    /// Performs the analysis and records the run as `Analyzed`.
    async fn detect(
        &self,
        conn: &mut PgConn,
        job: &DetectionJob,
        run: &WorkspacePipelineRun,
        pipeline: &WorkspacePipeline,
        claim_token: jiff::Timestamp,
    ) -> Result<()> {
        let workspace = conn
            .find_workspace_by_id(job.workspace_id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_message("Workspace not found"))?;
        let file = conn
            .find_file_in_workspace(job.workspace_id, run.input_file_id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_message("Input file not found"))?;

        let definition = PipelineDefinition::from_parts(pipeline.definition.clone(), Vec::new())
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to decode pipeline definition")
                    .with_context(err.to_string())
            })?;

        // Parse the workspace settings once; both OCR mode and retention read it.
        let settings = workspace.settings.or_default();
        let request =
            self.engine
                .request_context(&definition, job.scope.clone(), raster_mode_of(&settings));

        let document = self.blob.build_document(&file, run.id).await?;

        let policies =
            resolve_policies(conn, &self.infra.crypto, job.workspace_id, pipeline.id).await?;
        if policies.is_empty() {
            return Err(ErrorKind::BadRequest
                .with_message("Pipeline has no policies")
                .with_resource("pipeline"));
        }

        let analyzed = self.engine.analyze(document, &policies, &request).await?;

        // Write the (non-transactional) audit object first, then commit its file
        // row together with the run's usage and status in one transaction below.
        let audit_file = self
            .blob
            .stage_analyzed_document(pipeline, &settings.retention, run.account_id, &analyzed)
            .await?;

        // Record inference usage: per-model token rows into the usage table (the
        // usage aggregation surface) and the full per-recognizer report into
        // metadata for drill-down. Absent for a purely deterministic run. The
        // report is layered onto the run's existing metadata so tags and any
        // recorded error survive the write.
        let usage = extract_run_usage(run.id, &analyzed);
        let metadata = usage.as_ref().map(|u| {
            let mut current = run.metadata.or_default();
            current.usage = Some(u.report.clone());
            Json::encode(&current)
        });

        // Persist the audit file row, per-model usage, and the run's transition to
        // `Analyzed` atomically: a partial failure would otherwise strand usage
        // rows or an audit pointer on a run still marked `Analyzing`. The finalize
        // is fenced on our claim; if it went stale (another worker re-claimed the
        // run past the lease), the whole transaction rolls back so we do not stamp
        // over the new owner's work or leak usage/audit rows for a run we lost.
        // Kept to reclaim the just-staged object if the transaction does not
        // commit: on rollback its `workspace_files` row never lands, so the
        // row-driven reaper could never find the object otherwise.
        // Build the outbox row here so the finalize transaction is `PgError`-typed
        // for its rollback sentinel, and insert it alongside the finalize so the
        // `Analyzed` event commits atomically with the run.
        let analyzed_event = WorkspaceEvent::PipelineRunAnalyzed {
            run: PipelineRunRef {
                run_id: run.id,
                pipeline_slug: pipeline.slug.clone(),
            },
            input_file_name: Some(file.display_name.clone()),
            notify: run.account_id,
        };
        let outbox_row = event_outbox_row(
            EventOrigin {
                workspace_id: job.workspace_id,
                account_id: run.account_id,
                security: &SecurityContext::default(),
            },
            &analyzed_event,
        )?;

        let staged_audit = audit_file.clone();
        let finalized = conn
            .transaction(async |conn| {
                let audit_file_id = conn.create_workspace_file(audit_file).await?.id;
                if let Some(usage) = &usage {
                    conn.record_run_usage(&usage.per_model).await?;
                }
                let finalized = conn
                    .finalize_analyzed_run(
                        run.id,
                        claim_token,
                        UpdateWorkspacePipelineRun {
                            audit_file_id: Some(Some(audit_file_id)),
                            metadata,
                            ..Default::default()
                        },
                    )
                    .await?;
                if !finalized {
                    // Abort the audit-file and usage inserts: the run is no longer
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
                self.discard_staged_audit(&staged_audit).await;
                tracing::warn!(target: TRACING_TARGET, run_id = %run.id, "Claim went stale before finalize; another worker owns the run");
                return Ok(());
            }
            Err(err) => {
                // The transaction rolled back, so the audit row never committed;
                // reclaim its object before surfacing the failure.
                self.discard_staged_audit(&staged_audit).await;
                return Err(err.into());
            }
        }

        tracing::info!(target: TRACING_TARGET, run_id = %run.id, "Run analyzed");
        self.detection
            .broadcast_status(run.id, PipelineRunStatus::Analyzed)
            .await;

        Ok(())
    }
}

/// Whether a consumed detection job should be acked (done) or nacked (retry).
#[derive(Debug, Clone, Copy)]
enum JobOutcome {
    /// Reached a terminal outcome or is safe to drop; ack the message.
    Done,
    /// Transient error before the run was claimed; nack for redelivery.
    Retry,
}

/// Maps a workspace's OCR policy to the engine's per-run page-rasterisation
/// mode.
fn raster_mode_of(settings: &WorkspaceSettings) -> RasterMode {
    match settings.ocr {
        OcrPolicy::Auto => RasterMode::Auto,
        OcrPolicy::Force => RasterMode::always(),
        OcrPolicy::Never => RasterMode::Never,
    }
}
