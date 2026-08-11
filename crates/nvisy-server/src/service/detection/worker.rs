//! Pipeline detection worker.
//!
//! Consumes [`DetectionJob`]s from the `DetectionStream` work-queue and runs a
//! run's detection (analyze) in the background: builds the document, analyzes it
//! with the pipeline's policies, stores the encrypted audit, and marks the run
//! `Analyzed` (or `Failed`). Each terminal transition is broadcast on the run's
//! core-NATS status subject (for SSE watchers) and emitted as a webhook event.

use std::time::Duration;

use nvisy_engine::policy::PolicyDefinition;
use nvisy_nats::NatsClient;
use nvisy_nats::stream::DetectionStream;
use nvisy_postgres::model::{UpdateWorkspacePipelineRun, WorkspacePipeline, WorkspacePipelineRun};
use nvisy_postgres::query::{
    PipelineReferenceRepository, WorkspaceFileRepository, WorkspacePipelineRunRepository,
    WorkspacePolicyRepository, WorkspaceRepository,
};
use nvisy_engine::OcrMode;
use nvisy_postgres::types::{OcrPolicy, PipelineRunStatus, WorkspaceSettings};
use nvisy_postgres::{PgClient, PgConn};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::job::{DetectionJob, RunStatusEvent, run_subject};
use crate::handler::request::PipelineDefinition;
use crate::handler::{ErrorKind, Result};
use crate::service::{BlobService, CryptoService, EngineService, WebhookEmitter};

/// Tracing target for detection worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::detection";

/// Background worker that runs pipeline detection off the request thread.
pub struct DetectionWorker {
    postgres: PgClient,
    nats: NatsClient,
    crypto: CryptoService,
    engine: EngineService,
    blob: BlobService,
    webhook_emitter: WebhookEmitter,
}

impl DetectionWorker {
    /// Creates a new [`DetectionWorker`].
    pub fn new(
        postgres: PgClient,
        nats: NatsClient,
        crypto: CryptoService,
        engine: EngineService,
        blob: BlobService,
        webhook_emitter: WebhookEmitter,
    ) -> Self {
        Self {
            postgres,
            nats,
            crypto,
            engine,
            blob,
            webhook_emitter,
        }
    }

    /// Runs the worker until cancelled, logging its lifecycle.
    pub async fn run(&self, cancel: CancellationToken) -> Result<()> {
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

    /// Consumes detection jobs until cancelled. At-least-once: run first, then
    /// ack; a crash before ack redelivers, and the job is idempotent (a run no
    /// longer `Running` is skipped).
    async fn run_inner(&self, cancel: CancellationToken) -> Result<()> {
        let subscriber = self.nats.event_subscriber::<DetectionJob, DetectionStream>().await?;
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
                            self.run_job(job).await;
                            if let Err(err) = message.ack().await {
                                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to ack detection job");
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

    /// Runs one detection job: loads the run, analyzes, and records the result.
    #[tracing::instrument(skip_all, fields(run_id = %job.run_id, workspace_id = %job.workspace_id))]
    async fn run_job(&self, job: DetectionJob) {
        let mut conn = match self.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to get connection for detection job");
                return;
            }
        };

        let (run, pipeline) = match conn.find_workspace_run_by_id(job.workspace_id, job.run_id).await {
            Ok(Some(pair)) => pair,
            Ok(None) => {
                tracing::warn!(target: TRACING_TARGET, "Detection job for a missing run; skipping");
                return;
            }
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to load run for detection job");
                return;
            }
        };

        // Idempotent redelivery: only a still-`Running` run is analyzed.
        if run.status != PipelineRunStatus::Running {
            tracing::debug!(target: TRACING_TARGET, status = %run.status, "Run is not running; skipping detection");
            return;
        }

        if let Err(err) = self.detect(&mut conn, &job, &run, &pipeline).await {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Detection failed");
            self.mark_failed(&mut conn, &job, &run).await;
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

        let ocr_mode = ocr_mode_of(&workspace.settings);
        let params = self
            .engine
            .analyzer_params(&definition, job.scope.clone(), ocr_mode);

        let document = self.blob.build_document(&file, run.id).await?;

        let policies = resolve_policies(conn, &self.crypto, job.workspace_id, pipeline.id).await?;
        if policies.is_empty() {
            return Err(ErrorKind::BadRequest
                .with_message("Pipeline has no policies")
                .with_resource("pipeline"));
        }

        let analyzed = self.engine.analyze(document, &policies, &params).await?;

        let retention = WorkspaceSettings::from_value(&workspace.settings).retention;
        let audit_file_id = self
            .blob
            .store_analyzed_document(conn, pipeline, &retention, run.account_id, &analyzed)
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
        self.broadcast_status(run.id, PipelineRunStatus::Analyzed).await;

        if let Err(err) = self
            .webhook_emitter
            .emit_pipeline_run_analyzed(job.workspace_id, run.id, Some(run.account_id), None)
            .await
        {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Failed to emit pipeline:run.analyzed webhook event");
        }

        Ok(())
    }

    /// Marks a run `Failed`, broadcasts, and emits the failure webhook.
    async fn mark_failed(&self, conn: &mut PgConn, job: &DetectionJob, run: &WorkspacePipelineRun) {
        let update = UpdateWorkspacePipelineRun {
            status: Some(PipelineRunStatus::Failed),
            completed_at: Some(Some(jiff::Timestamp::now().into())),
            ..Default::default()
        };
        if let Err(err) = conn.update_workspace_pipeline_run(run.id, update).await {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Failed to mark run failed");
        }

        self.broadcast_status(run.id, PipelineRunStatus::Failed).await;

        if let Err(err) = self
            .webhook_emitter
            .emit_pipeline_run_failed(job.workspace_id, run.id, Some(run.account_id), None)
            .await
        {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Failed to emit pipeline:run.failed webhook event");
        }
    }

    /// Broadcasts a run's status change on its core-NATS subject (best-effort).
    async fn broadcast_status(&self, run_id: Uuid, status: PipelineRunStatus) {
        let event = RunStatusEvent { run_id, status };
        if let Err(err) = self.nats.publish_broadcast(run_subject(run_id), &event).await {
            tracing::debug!(target: TRACING_TARGET, error = %err, "Failed to broadcast run status");
        }
    }
}

/// Resolves a pipeline's live policy references into decrypted engine policies.
pub(crate) async fn resolve_policies(
    conn: &mut PgConn,
    crypto: &CryptoService,
    workspace_id: Uuid,
    pipeline_id: Uuid,
) -> Result<Vec<PolicyDefinition>> {
    let ids = conn.list_pipeline_policy_ids(pipeline_id).await?;
    let mut policies = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(model) = conn.find_policy_in_workspace(workspace_id, id).await? {
            policies.push(crypto.decrypt_json::<PolicyDefinition>(workspace_id, &model.definition)?);
        }
    }
    Ok(policies)
}

/// Maps a workspace's OCR policy to the engine's per-run OCR mode.
pub(crate) fn ocr_mode_of(settings: &serde_json::Value) -> OcrMode {
    match WorkspaceSettings::from_value(settings).ocr {
        OcrPolicy::Auto => OcrMode::Auto,
        OcrPolicy::Force => OcrMode::force(),
        OcrPolicy::Never => OcrMode::Never,
    }
}
