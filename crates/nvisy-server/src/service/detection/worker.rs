//! Pipeline detection worker.
//!
//! Consumes [`DetectionJob`]s from the `DetectionStream` work-queue and runs a
//! run's detection (analyze) in the background: builds the document, analyzes it
//! with the pipeline's policies, stores the encrypted audit, and marks the run
//! `Analyzed` (or `Failed`). Each terminal transition is broadcast on the run's
//! core-NATS status subject (for SSE watchers) and emitted as a webhook event.

use std::time::Duration;

use nvisy_engine::OcrMode;
use nvisy_nats::stream::DetectionStream;
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{UpdateWorkspacePipelineRun, WorkspacePipeline, WorkspacePipelineRun};
use nvisy_postgres::query::{
    WorkspaceFileRepository, WorkspacePipelineRunRepository, WorkspaceRepository,
};
use nvisy_postgres::types::{
    NotificationPayload, OcrPolicy, PipelineRunAnalyzedParams, PipelineRunFailedParams,
    PipelineRunStatus, WorkspaceSettings,
};
use tokio_util::sync::CancellationToken;

use super::job::DetectionJob;
use super::service::DetectionQueue;
use super::support::{fail_run, resolve_policies};
use crate::handler::request::PipelineDefinition;
use crate::handler::{ErrorKind, Result};
use crate::service::{
    EngineService, Infra, NotificationEmitter, RunBlobStore, WebhookEmitter, Worker,
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
    webhook_emitter: WebhookEmitter,
    notification_emitter: NotificationEmitter,
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
        webhook_emitter: WebhookEmitter,
        notification_emitter: NotificationEmitter,
        detection: DetectionQueue,
    ) -> Self {
        Self {
            infra,
            engine,
            blob,
            webhook_emitter,
            notification_emitter,
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
            Ok(Some(_)) => true,
            Ok(None) => {
                tracing::debug!(target: TRACING_TARGET, "Run already claimed by another worker; skipping");
                return JobOutcome::Done;
            }
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to claim run for detection");
                return JobOutcome::Retry;
            }
        };
        debug_assert!(claimed);

        self.detection
            .broadcast_status(run.id, PipelineRunStatus::Analyzing)
            .await;

        if let Err(err) = self.detect(&mut conn, &job, &run, &pipeline).await {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Detection failed");
            fail_run(
                &mut conn,
                &self.detection,
                &self.webhook_emitter,
                job.workspace_id,
                run.id,
                run.account_id,
                &err.to_string(),
            )
            .await;

            let payload = NotificationPayload::PipelineRunFailed(PipelineRunFailedParams {
                run_id: run.id,
                pipeline_slug: pipeline.slug.to_string(),
                input_file_name: None,
                error: Some(err.to_string()),
            });
            self.notify(job.workspace_id, run.account_id, payload).await;
        }
        JobOutcome::Done
    }

    /// Notifies the run's triggering account, logging (never failing) on error.
    async fn notify(
        &self,
        workspace_id: uuid::Uuid,
        account_id: uuid::Uuid,
        payload: NotificationPayload,
    ) {
        if let Err(err) = self
            .notification_emitter
            .notify_account(workspace_id, account_id, payload)
            .await
        {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Failed to create notification");
        }
    }

    /// Performs the analysis and records the run as `Analyzed`.
    async fn detect(
        &self,
        conn: &mut PgConn,
        job: &DetectionJob,
        run: &WorkspacePipelineRun,
        pipeline: &WorkspacePipeline,
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
        let params =
            self.engine
                .analyzer_params(&definition, job.scope.clone(), ocr_mode_of(&settings));

        let document = self.blob.build_document(&file, run.id).await?;

        let policies =
            resolve_policies(conn, &self.infra.crypto, job.workspace_id, pipeline.id).await?;
        if policies.is_empty() {
            return Err(ErrorKind::BadRequest
                .with_message("Pipeline has no policies")
                .with_resource("pipeline"));
        }

        let analyzed = self.engine.analyze(document, &policies, &params).await?;

        let audit_file_id = self
            .blob
            .store_analyzed_document(
                conn,
                pipeline,
                &settings.retention,
                run.account_id,
                &analyzed,
            )
            .await?;

        conn.update_workspace_pipeline_run(
            run.id,
            UpdateWorkspacePipelineRun {
                status: Some(PipelineRunStatus::Analyzed),
                audit_file_id: Some(Some(audit_file_id)),
                ..Default::default()
            },
        )
        .await?;

        tracing::info!(target: TRACING_TARGET, run_id = %run.id, "Run analyzed");
        self.detection
            .broadcast_status(run.id, PipelineRunStatus::Analyzed)
            .await;

        if let Err(err) = self
            .webhook_emitter
            .emit_pipeline_run_analyzed(job.workspace_id, run.id, Some(run.account_id), None)
            .await
        {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Failed to emit pipeline:run.analyzed webhook event");
        }

        let payload = NotificationPayload::PipelineRunAnalyzed(PipelineRunAnalyzedParams {
            run_id: run.id,
            pipeline_slug: pipeline.slug.to_string(),
            input_file_name: Some(file.display_name.clone()),
        });
        self.notify(job.workspace_id, run.account_id, payload).await;

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

/// Maps a workspace's OCR policy to the engine's per-run OCR mode.
fn ocr_mode_of(settings: &WorkspaceSettings) -> OcrMode {
    match settings.ocr {
        OcrPolicy::Auto => OcrMode::Auto,
        OcrPolicy::Force => OcrMode::force(),
        OcrPolicy::Never => OcrMode::Never,
    }
}
