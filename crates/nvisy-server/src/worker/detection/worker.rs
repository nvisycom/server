//! Pipeline detection worker.
//!
//! Consumes [`DetectionJob`]s from the `DetectionStream` work-queue and runs a
//! detection's analysis in the background: builds the document, analyzes it with
//! the pipeline's policies, stores the encrypted audit, and marks the detection
//! `Complete` (or `Failed`). Each terminal transition is broadcast on the
//! detection's core-NATS status subject (for SSE watchers) and emitted as a
//! webhook event.

use std::sync::Arc;
use std::time::Duration;

use elide_pipeline::primitive::RasterMode;
use elide_pipeline::provider::{CodecParams, DocumentContext, RequestContext};
use nvisy_postgres::model::{
    NewBlob, NewWorkspaceAudit, UpdateWorkspaceDetection, WorkspaceDetection, WorkspacePipeline,
};
use nvisy_postgres::query::{
    DetectionPolicyVersionRepository, EventOutboxRepository, WorkspaceAuditRepository,
    WorkspaceBlobRepository, WorkspaceDetectionRepository, WorkspaceDocumentRepository,
    WorkspaceRepository,
};
use nvisy_postgres::types::{
    DetectionStatus, Json, RasterPolicy, RetentionOverride, WorkspaceSettings,
};
use nvisy_postgres::{AsyncConnection, DieselError, Error as PgError};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::job::{DetectionJob, DetectionStream, broadcast_status};
use super::support::{
    FailDetection, FailOutcome, extract_detection_usage, fail_detection, resolve_policies,
    resolve_policies_by_ids,
};
use crate::extract::SecurityContext;
use crate::handler::request::PipelineDefinition;
use crate::response::{ErrorKind, Result};
use crate::service::{EngineService, Infra, RunBlobStore, event};
use crate::worker::Worker;

/// Tracing target for detection worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::detection";

/// How long a detection claim stays valid before another delivery may re-claim
/// the detection. Set above `DetectionStream::ACK_WAIT` (15 min) so a
/// slow-but-healthy worker whose job is redelivered keeps its claim; only a
/// detection whose worker died (no progress past the lease) is re-claimed and
/// re-analyzed.
const DETECTION_LEASE: Duration = Duration::from_mins(30);

/// Background worker that runs pipeline detection off the request thread.
///
/// Cheaply cloneable (every field is `Arc`-backed); a clone is handed to each
/// spawned per-job task so jobs run concurrently against the shared services.
#[derive(Clone)]
pub struct DetectionWorker {
    infra: Infra,
    engine: EngineService,
    blob: RunBlobStore,
    /// Bounds how many detection jobs run at once, sized to the deployment's
    /// available parallelism. Detection analysis is CPU-bound under the default
    /// lineup, so unbounded concurrency would only oversubscribe cores and grow
    /// per-job latency; the semaphore keeps in-flight jobs near core count.
    concurrency: Arc<Semaphore>,
}

/// Fallback concurrency when the runtime cannot report available parallelism.
const DEFAULT_DETECTION_CONCURRENCY: usize = 4;

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
                tracing::error!(target: TRACING_TARGET, error = %err, "Detection worker failed");
            }
        }

        result
    }
}

impl DetectionWorker {
    /// Creates a new `DetectionWorker`.
    ///
    /// Concurrency is sized to the deployment's available parallelism (falling
    /// back to a small default when the runtime cannot report it), so in-flight
    /// detections stay near core count.
    #[must_use]
    pub fn new(infra: Infra, engine: EngineService, blob: RunBlobStore) -> Self {
        let concurrency = std::thread::available_parallelism()
            .map_or(DEFAULT_DETECTION_CONCURRENCY, std::num::NonZero::get);
        Self {
            infra,
            engine,
            blob,
            concurrency: Arc::new(Semaphore::new(concurrency)),
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
        let subscriber = self.infra.nats.event_subscriber::<DetectionStream>();
        let mut stream = subscriber.subscribe().await?;

        // In-flight per-job tasks are owned here rather than detached, so shutdown
        // can wait for them to settle their message (ack/nack) instead of the
        // worker reporting stopped while a task still runs. `JoinSet` also reaps
        // finished tasks so the set does not grow unbounded.
        let mut tasks: JoinSet<()> = JoinSet::new();

        loop {
            // Acquire a permit before pulling the next job so no more than
            // `concurrency` detections are ever in flight; the pull, and thus the
            // stream's redelivery lease, does not advance while every worker slot
            // is busy. The semaphore is never closed, so acquire only errors on a
            // closed semaphore — treat that as fatal for the loop.
            let permit = tokio::select! {
                () = cancel.cancelled() => {
                    tracing::info!(target: TRACING_TARGET, "Detection worker shutdown requested");
                    break;
                }
                // Reap completed tasks as they finish, so the set stays bounded by
                // the number actually in flight rather than by all jobs ever run.
                Some(_) = tasks.join_next() => continue,
                permit = self.concurrency.clone().acquire_owned() => match permit {
                    Ok(permit) => permit,
                    Err(_) => break,
                },
            };

            tokio::select! {
                () = cancel.cancelled() => {
                    tracing::info!(target: TRACING_TARGET, "Detection worker shutdown requested");
                    break;
                }
                result = stream.next_with_timeout(Duration::from_secs(5)) => {
                    match result {
                        Ok(Some(mut message)) => {
                            let job = message.payload().clone();
                            // Run each job on its own task so a slow detection does
                            // not block the next pull; the permit is moved in and
                            // released when the task finishes. The worker is cheaply
                            // cloneable (all services are `Arc`-backed).
                            let worker = self.clone();
                            tasks.spawn(async move {
                                let _permit = permit;
                                // reason: detection job future is inherently large; boxing would only move the allocation
                                #[allow(clippy::large_futures)]
                                let outcome = worker.run_job(job).await;
                                let ack_result = match outcome {
                                    JobOutcome::Done => message.ack().await,
                                    // Transient failure: redeliver instead of
                                    // dropping the job, so the run is eventually
                                    // settled.
                                    JobOutcome::Retry => message.nack().await,
                                };
                                if let Err(err) = ack_result {
                                    tracing::error!(target: TRACING_TARGET, error = %err, ?outcome, "Failed to ack/nack detection job");
                                }
                            });
                        }
                        // No job before the timeout: drop the permit and pull again.
                        Ok(None) => drop(permit),
                        Err(err) => {
                            drop(permit);
                            tracing::error!(target: TRACING_TARGET, error = %err, "Error receiving detection job");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }

        // Shutdown: stop pulling new jobs and let the in-flight ones finish so each
        // settles its message (ack/nack) rather than being abandoned mid-run. The
        // app-wide shutdown timeout (`WorkerSet::shutdown`) bounds how long this
        // can take; if it fires, any task still running is aborted and its message
        // is redelivered, which the claim makes idempotent.
        if !tasks.is_empty() {
            tracing::info!(
                target: TRACING_TARGET,
                in_flight = tasks.len(),
                "Draining in-flight detection jobs before stopping",
            );
            while tasks.join_next().await.is_some() {}
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

        broadcast_status(&self.infra, detection.id, DetectionStatus::Executing).await;

        // Release the connection before analysis: `detect` manages its own
        // connections across its phases, so holding this one across the (slow)
        // inference would pin a pooled connection per in-flight job and starve the
        // pool. It is re-acquired below only if the detection fails.
        drop(conn);

        if let Err(err) = self
            .detect(&job, &claimed, pipeline.as_ref(), claim_token)
            .await
        {
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
                &self.infra,
                FailDetection {
                    workspace_id: job.workspace_id,
                    detection_id: detection.id,
                    // The detection's own `pipeline_id` is durable across a
                    // pipeline soft-delete, whereas the `pipeline` lookup excludes
                    // a soft-deleted row and would drop the id from the event.
                    pipeline_id: detection.pipeline_id,
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

    /// Best-effort reclaim of a staged object whose blob did not commit. A failure
    /// only defers cleanup, so it is logged, never propagated.
    async fn discard_staged(&self, staged: &NewBlob) {
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
    /// analysis ran no enricher for any group (its artifact set serializes empty).
    /// Skipping the empty case avoids an intermediates blob that a client would
    /// fetch only to find nothing.
    async fn stage_intermediates<T: serde::Serialize>(
        &self,
        workspace_id: Uuid,
        retention_override: Option<&RetentionOverride>,
        settings: &WorkspaceSettings,
        artifacts: &T,
    ) -> Result<Option<NewBlob>> {
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

        let blob = self
            .blob
            .stage_intermediates(
                workspace_id,
                retention_override,
                &settings.retention,
                artifacts,
            )
            .await?;
        Ok(Some(blob))
    }

    /// Performs the analysis and records the detection as `Complete`.
    ///
    /// Manages its own connection lifecycle in three phases so a pooled
    /// connection is never held across the analysis inference: phase 1 reads the
    /// inputs under a connection and releases it, phase 2 runs the (slow) document
    /// build, analysis, and audit staging with no connection held, and phase 3
    /// re-acquires a connection only for the finalize transaction.
    // reason: long but cohesive; splitting adds no clarity
    #[allow(clippy::too_many_lines)]
    async fn detect(
        &self,
        job: &DetectionJob,
        detection: &WorkspaceDetection,
        pipeline: Option<&WorkspacePipeline>,
        claim_token: jiff::Timestamp,
    ) -> Result<()> {
        // Phase 1: read the inputs under a connection, then drop it.
        let (document, blob, request, policies, policy_version_ids, settings) = {
            let mut conn = self.infra.postgres.get_connection().await?;

            let workspace = conn
                .find_workspace_by_id(job.workspace_id)
                .await?
                .ok_or_else(|| ErrorKind::NotFound.with_message("Workspace not found"))?;
            let document = conn
                .find_document_in_workspace(job.workspace_id, detection.input_document_id)
                .await?
                .ok_or_else(|| ErrorKind::NotFound.with_message("Input document not found"))?;
            let blob = conn
                .find_blob_by_id(document.blob_id)
                .await?
                .ok_or_else(|| {
                    ErrorKind::NotFound.with_message("Input document content not found")
                })?;

            // A pipeline detection takes its default scope from the pipeline
            // definition; an ad-hoc detection has none, so the engine falls back to
            // the per-request scope and its own defaults.
            let definition = match pipeline {
                Some(pipeline) => {
                    PipelineDefinition::from_parts(pipeline.definition.clone(), Vec::new())
                        .map_err(|err| {
                            ErrorKind::InternalServerError
                                .with_message("Failed to decode pipeline definition")
                                .with_context(err.to_string())
                        })?
                }
                None => PipelineDefinition::default(),
            };

            // Parse the workspace settings once; both raster mode and retention
            // read it.
            let settings = workspace.settings.or_default();
            let request =
                request_context(&definition, job.scope.clone(), raster_mode_of(&settings));

            // Resolve the policies the analysis runs against, keyed off the
            // detection's own `pipeline_id` (stable across a pipeline soft-delete,
            // which nulls nothing) rather than the live pipeline row: a pipeline
            // detection resolves from the pipeline's references — still reachable
            // through the join rows a soft-delete leaves in place — and an ad-hoc
            // detection from the ids named on the job.
            let resolved = match detection.pipeline_id {
                Some(pipeline_id) => {
                    resolve_policies(&mut conn, job.workspace_id, pipeline_id).await?
                }
                None => {
                    resolve_policies_by_ids(&mut conn, job.workspace_id, &job.policy_ids).await?
                }
            };
            if resolved.is_empty() {
                return Err(ErrorKind::BadRequest.with_message("Detection has no policies"));
            }
            // Split the resolved set into the version ids the run pins and the
            // definitions the engine consumes.
            let mut policy_version_ids = Vec::with_capacity(resolved.len());
            let mut policies = Vec::with_capacity(resolved.len());
            for policy in resolved {
                policy_version_ids.push(policy.version_id);
                policies.push(policy.definition);
            }

            (
                document,
                blob,
                request,
                policies,
                policy_version_ids,
                settings,
            )
        };

        // Phase 2: the slow work — document build, analysis inference, and audit
        // staging — runs with no DB connection held. Analysis runs on a blocking
        // thread (`analyze_blocking`): with the local recognizer lineup it is
        // CPU-bound and would otherwise pin an async worker thread for the whole
        // analysis, so keeping it off the async pool lets many detections run at
        // once without starving the rest of the server.
        let engine_document = self
            .blob
            .build_document(&document, &blob, detection.id)
            .await?;
        let analyzed = self
            .engine
            .analyze_blocking(engine_document, policies, request)
            .await?;
        let audit = &analyzed.audit;

        // Write the (non-transactional) audit object first, then commit its blob
        // and base-audit row together with the detection's usage and status in one
        // transaction below.
        let retention_override = detection
            .retention_override
            .as_ref()
            .map(nvisy_postgres::types::Json::or_default);
        let audit_blob = self
            .blob
            .stage_analyzed_document(
                detection.workspace_id,
                retention_override.as_ref(),
                &settings.retention,
                audit,
            )
            .await?;

        // Stage the enrichment intermediates (OCR layout, transcript, tokenized
        // text) beside the audit, so the client can read them and add entities the
        // analysis missed. An analysis that ran no enricher produces an empty
        // artifact set — nothing is stored and the detection carries no
        // intermediates reference.
        let intermediates_blob = self
            .stage_intermediates(
                detection.workspace_id,
                retention_override.as_ref(),
                &settings,
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
        // `workspace_blobs` row never lands, so the blob-driven reaper could never
        // find the object otherwise. Build the outbox row here so the finalize
        // transaction is `PgError`-typed for its rollback sentinel, and insert it
        // alongside the finalize so the `Complete` event commits atomically with
        // the detection.
        let completed_event =
            event::WorkspaceEvent::DetectionCompleted(event::DetectionCompleted {
                detection_id: detection.id,
                // Durable across a pipeline soft-delete; see the fail path.
                pipeline_id: detection.pipeline_id,
                input_document_name: Some(document.display_name.clone()),
                notify: detection.account_id,
            });
        let outbox_row = event::event_outbox_row(
            event::EventOrigin {
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
        // on rollback their `workspace_blobs` rows never land, so the blob-driven
        // reaper could never find the objects otherwise.
        let staged_audit = audit_blob.clone();
        let staged_intermediates = intermediates_blob.clone();
        let detection_id = detection.id;
        let workspace_id = job.workspace_id;
        let finalized = conn
            .transaction(async |conn| {
                // The base audit resolves (shares or inserts) its blob and records
                // the reference in this same transaction.
                let audit = conn
                    .create_audit(
                        NewWorkspaceAudit::base(workspace_id, Uuid::nil(), detection_id),
                        audit_blob,
                    )
                    .await?;
                // Pin the exact policy versions this analysis consumed, so the
                // detection is reproducible against them regardless of later edits.
                conn.record_detection_policy_versions(detection_id, &policy_version_ids)
                    .await?;
                // The intermediate is a blob-ref on the detection; resolving the
                // blob records the detection's reference to it.
                let resolved_blob = match intermediates_blob {
                    Some(blob) => Some(conn.find_or_create_blob(blob).await?),
                    None => None,
                };
                let intermediate_blob_id = resolved_blob.as_ref().map(|blob| blob.id);
                if let Some(usage) = &usage {
                    conn.record_detection_usage(&usage.per_model).await?;
                }
                let finalized = conn
                    .finalize_detection(
                        detection.id,
                        claim_token,
                        UpdateWorkspaceDetection {
                            intermediate_blob_id: Some(intermediate_blob_id),
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
                // The resolved storage paths, so an object staged for content that
                // deduplicated onto an existing blob can be reclaimed after commit.
                let intermediate_path = resolved_blob.map(|blob| blob.storage_path);
                Ok::<_, PgError>((audit.blob_id, intermediate_path))
            })
            .await;

        match finalized {
            Ok((audit_blob_id, intermediate_path)) => {
                // A base audit or intermediate whose content deduplicated onto an
                // existing blob is pointed at that blob's stored object, orphaning
                // the object staged for it. Reclaim the redundant staged objects.
                if let Ok(Some(blob)) = conn.find_blob_by_id(audit_blob_id).await
                    && blob.storage_path != staged_audit.storage_path
                {
                    self.discard_staged(&staged_audit).await;
                }
                if let Some(staged) = &staged_intermediates
                    && intermediate_path.as_deref() != Some(staged.storage_path.as_str())
                {
                    self.discard_staged(staged).await;
                }
            }
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
        broadcast_status(&self.infra, detection.id, DetectionStatus::Complete).await;

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

/// Builds the [`RequestContext`] for one detect run from a pipeline's intent.
///
/// Recognition is entirely engine-owned (the built-in pattern set plus the
/// deployment's NER/LLM lineups always run). The document context is the
/// request's own, falling back to the pipeline default. `raster_mode` is the
/// workspace's page-rasterisation policy (always render vs. auto), carried in the
/// codec params since it is server-derived, not caller-set. The engine records
/// the context and codec params on the audit so redaction re-decodes and
/// re-compiles against exactly what detection used. Deduplication and calibration
/// are engine-owned defaults; the label catalog is derived from the run's
/// policies at detect time.
///
/// No key is set: the server does not yet drive keyed operators
/// (`HmacHash`/`Encrypt`), whose `KeyConfig` would be supplied here.
fn request_context(
    definition: &PipelineDefinition,
    document_context: Option<DocumentContext>,
    raster_mode: RasterMode,
) -> RequestContext {
    let context = document_context
        .or_else(|| definition.default_scope.clone())
        .unwrap_or_default();
    RequestContext::new()
        .with_context(context)
        .with_codec(CodecParams::new().with_raster_mode(raster_mode))
}
