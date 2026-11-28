//! Detection worker.
//!
//! Consumes [`DetectionJob`]s from the `DetectionStream` work-queue and runs a
//! detection's analysis in the background: builds the document, analyzes it with
//! the detection's policies, stores the encrypted audit, and marks the detection
//! `Complete` (or `Failed`). Each terminal transition is broadcast on the
//! detection's core-NATS status subject (for SSE watchers) and emitted as a
//! webhook event.

use std::iter;
use std::num::NonZero;
use std::time::Duration;

use elide_pipeline::ArtifactSet;
use elide_pipeline::primitive::RasterMode;
use elide_pipeline::provider::{CodecParams, DocumentContext, RequestContext};
use nvisy_nats::stream::Settle;
use nvisy_postgres::model::{UpdateWorkspaceDetection, WorkspaceDetection};
use nvisy_postgres::query::{
    DetectionPolicyVersionRepository, OutboxRepository, WorkspaceBlobRepository,
    WorkspaceDetectionRepository, WorkspaceDocumentRepository, WorkspaceRepository,
};
use nvisy_postgres::types::{DetectionStatus, RasterPolicy, RetentionOverride, WorkspaceSettings};
use nvisy_postgres::{AsyncConnection, DieselError, Error as PgError};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::job::{DetectionJob, DetectionStream, broadcast_status};
use super::support::{FailDetection, FailOutcome, fail_detection, resolve_policies_by_ids};
use crate::extract::SecurityContext;
use crate::response::{Error, ErrorKind, Result};
use crate::service::{ArtifactReader, ArtifactWriter, EngineService, Infra, StagedBlob, event};
use crate::worker::Worker;

/// Tracing target for detection worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::detection";

/// How long a detection claim stays valid before another delivery may re-claim
/// the detection. Set above `DetectionStream::ACK_WAIT` (15 min) so a
/// slow-but-healthy worker whose job is redelivered keeps its claim; only a
/// detection whose worker died (no progress past the lease) is re-claimed and
/// re-analyzed.
const DETECTION_LEASE: Duration = Duration::from_mins(30);

/// Background worker that runs detection off the request thread.
///
/// Cheaply cloneable (every field is `Arc`-backed); a clone is handed to each
/// spawned per-job task so jobs run concurrently against the shared services.
#[derive(Clone)]
pub struct DetectionWorker {
    infra: Infra,
    engine: EngineService,
    reader: ArtifactReader,
    writer: ArtifactWriter,
    /// How many detection jobs run at once, sized to the deployment's available
    /// parallelism. Detection analysis is CPU-bound under the default lineup, so
    /// unbounded concurrency would only oversubscribe cores and grow per-job
    /// latency.
    concurrency: usize,
}

/// Fallback concurrency when the runtime cannot report available parallelism.
const DEFAULT_DETECTION_CONCURRENCY: usize = 4;

impl Worker for DetectionWorker {
    type Error = Error<'static>;

    const NAME: &'static str = "detection";

    /// At-least-once with an explicit claim: a job is acked once it reaches a
    /// terminal outcome (complete, or marked failed), and redelivered on a
    /// transient error (a DB/pool blip before the detection could even be
    /// claimed), so a detection is never silently stranded in a non-terminal
    /// state. The claim (`claim_detection`) makes redelivery idempotent: a
    /// detection already being analyzed under a fresh lease is skipped.
    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        let worker = self.clone();
        self.infra
            .nats
            .event_subscriber::<DetectionStream>()
            .consume(self.concurrency, &cancel, move |job, _delivery| {
                let worker = worker.clone();
                async move {
                    // reason: detection job future is inherently large; boxing would only move the allocation
                    #[allow(clippy::large_futures)]
                    let settle = worker.run_job(job).await;
                    settle
                }
            })
            .await?;
        Ok(())
    }
}

impl DetectionWorker {
    /// Creates a new `DetectionWorker`.
    ///
    /// Concurrency is sized to the deployment's available parallelism (falling
    /// back to a small default when the runtime cannot report it), so in-flight
    /// detections stay near core count.
    #[must_use]
    pub fn new(
        infra: Infra,
        engine: EngineService,
        reader: ArtifactReader,
        writer: ArtifactWriter,
    ) -> Self {
        let concurrency = std::thread::available_parallelism()
            .map_or(DEFAULT_DETECTION_CONCURRENCY, NonZero::get);
        Self {
            infra,
            engine,
            reader,
            writer,
            concurrency,
        }
    }

    /// Runs one detection job: claims the detection, analyzes, and records the
    /// result.
    ///
    /// Returns [`Settle::Retry`] on a transient error before the detection was
    /// claimed, [`Settle::RetryAfter`] the claim lease when a failure after the
    /// claim could not be recorded (so the redelivery can reclaim it), and
    /// [`Settle::Ack`] when it reached a terminal outcome or is safe to drop
    /// (missing detection, already claimed, already settled).
    #[tracing::instrument(skip_all, fields(detection_id = %job.detection_id, workspace_id = %job.workspace_id))]
    async fn run_job(&self, job: DetectionJob) -> Settle {
        let mut conn = match self.infra.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                // No connection: the detection is still pending. Redeliver so it
                // is not stranded in a non-terminal state.
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to get connection for detection job");
                return Settle::Retry;
            }
        };

        let detection = match conn
            .find_workspace_detection_by_id(job.workspace_id, job.detection_id)
            .await
        {
            Ok(Some(detection)) => detection,
            Ok(None) => {
                tracing::warn!(target: TRACING_TARGET, "Detection job for a missing detection; dropping");
                return Settle::Ack;
            }
            Err(err) => {
                // Transient load error: redeliver rather than strand the detection.
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to load detection for detection job");
                return Settle::Retry;
            }
        };

        // Nothing to do for a detection already past the pending/executing phase.
        if !detection.status.is_detecting() {
            tracing::debug!(target: TRACING_TARGET, status = %detection.status, "Detection is not detecting; dropping");
            return Settle::Ack;
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
                return Settle::Ack;
            }
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Failed to claim detection");
                return Settle::Retry;
            }
        };
        // The claim stamped `claimed_at`; it fences the finalize (and a failure)
        // against a concurrent re-claim if this analysis outlives the lease.
        let Some(claim_token) = claimed.claimed_at else {
            tracing::error!(target: TRACING_TARGET, "Claimed detection has no claim timestamp; skipping");
            return Settle::Ack;
        };
        let claim_token: jiff::Timestamp = claim_token.into();

        broadcast_status(&self.infra, detection.id, DetectionStatus::Executing).await;

        // Release the connection before analysis: `detect` manages its own
        // connections across its phases, so holding this one across the (slow)
        // inference would pin a pooled connection per in-flight job and starve the
        // pool. It is re-acquired below only if the detection fails.
        drop(conn);

        if let Err(err) = self.detect(&job, &claimed, claim_token).await {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Detection failed");
            let mut conn = match self.infra.postgres.get_connection().await {
                Ok(conn) => conn,
                Err(err) => {
                    // No connection to persist the failure: the detection stays
                    // `Executing` with no queued job, so redeliver once this
                    // attempt's claim lease lapses, when a later attempt can
                    // reclaim it and drive it to a terminal state.
                    tracing::error!(target: TRACING_TARGET, error = %err, "Failed to get connection to fail detection");
                    return Settle::RetryAfter(DETECTION_LEASE);
                }
            };
            let outcome = fail_detection(
                &mut conn,
                &self.infra,
                FailDetection {
                    workspace_id: job.workspace_id,
                    detection_id: detection.id,
                    triggered_by: detection.account_id,
                    reason: &err.to_string(),
                    metadata: detection.metadata.or_default(),
                    claim: Some(claim_token),
                },
            )
            .await;
            // If the failure state could not be persisted, the detection is left
            // `Executing` with no queued job to reclaim its lease: redeliver once
            // the lease lapses (an earlier redelivery would find it still claimed
            // and ack), so a later attempt drives it to a terminal state.
            if outcome == FailOutcome::PersistFailed {
                return Settle::RetryAfter(DETECTION_LEASE);
            }
        }
        Settle::Ack
    }

    /// Stages a detection's enrichment intermediates, or returns `None` when the
    /// analysis enriched no part (its artifact set is empty). Skipping the empty
    /// case avoids an intermediates blob that a client would fetch only to find
    /// nothing. A part enriched to an empty artifact (an image OCR'd to no text)
    /// still counts, so a restored re-run reuses it rather than re-enriching.
    async fn stage_intermediates(
        &self,
        workspace_id: Uuid,
        retention_override: Option<&RetentionOverride>,
        settings: &WorkspaceSettings,
        artifacts: &ArtifactSet,
    ) -> Result<Option<StagedBlob>> {
        if artifacts.is_empty() {
            return Ok(None);
        }

        let blob = self
            .writer
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
                .find_blob_by_id(document.blob_id.ok_or_else(|| {
                    ErrorKind::NotFound.with_message("Input document content not found")
                })?)
                .await?
                .ok_or_else(|| {
                    ErrorKind::NotFound.with_message("Input document content not found")
                })?;

            // Parse the workspace settings once; both raster mode and retention
            // read it.
            let settings = workspace.settings.or_default();
            let request = request_context(job.scope.clone(), raster_mode_of(&settings));

            // Resolve the policies the analysis runs against from the ids named on the
            // job (the detect request's policy list).
            let resolved =
                resolve_policies_by_ids(&mut conn, job.workspace_id, &job.policy_ids).await?;
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
            .reader
            .build_document(&document, &blob, detection.id)
            .await?;
        let analyzed = self
            .engine
            .analyze_blocking(engine_document, policies, request)
            .await?;
        let audit = &analyzed.audit;

        // Write the (non-transactional) audit object first, then commit its blob
        // and base-audit row together with the detection's status in one
        // transaction below.
        let retention_override = detection
            .retention_override
            .as_ref()
            .map(nvisy_postgres::types::Json::or_default);
        // The `Complete` event is built before anything is staged, so the only
        // failures between the first object write and the finalize are the ones
        // that discard what was staged. It is inserted alongside the finalize so
        // the event commits atomically with the detection, which also keeps the
        // finalize transaction `PgError`-typed for its rollback sentinel.
        let completed_event =
            event::WorkspaceEvent::DetectionCompleted(event::DetectionCompleted {
                detection_id: detection.id,
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

        let audit_blob = self
            .writer
            .stage_audit(
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
        let intermediates_blob = match self
            .stage_intermediates(
                detection.workspace_id,
                retention_override.as_ref(),
                &settings,
                &analyzed.artifacts,
            )
            .await
        {
            Ok(staged) => staged,
            Err(err) => {
                // The audit object is already written and has no blob row yet.
                self.writer.discard_staged(&[&audit_blob]).await;
                return Err(err);
            }
        };

        // Phase 3: re-acquire a connection only for the fenced finalize
        // transaction, so the pool was free during the analysis above. Without one
        // the staged objects have no blob row for the reaper to find, so discard
        // them before returning.
        let mut conn = match self.infra.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                self.writer
                    .discard_staged(&staged_outputs(&audit_blob, intermediates_blob.as_ref()))
                    .await;
                return Err(err.into());
            }
        };
        // Persist the audit file row and the detection's transition to
        // `Complete` atomically: a partial failure would otherwise strand an
        // audit pointer on a detection still marked `Executing`. The finalize is
        // fenced on our claim; if it went stale (another worker re-claimed the
        // detection past the lease), the whole transaction rolls back so we do
        // not stamp over the new owner's work or leak audit rows for a detection
        // we lost.
        // Kept to reclaim the staged objects if the transaction does not commit:
        // on rollback their `workspace_blobs` rows never land, so the blob-driven
        // reaper could never find the objects otherwise.
        let staged_audit = audit_blob.clone();
        let staged_intermediates = intermediates_blob.clone();
        let detection_id = detection.id;
        let finalized = conn
            .transaction(async |conn| {
                // The base analysis is a blob-ref on the detection: resolving
                // (sharing or inserting) its blob records the detection's reference,
                // and `audit_blob_id` is set on the detection at finalize below.
                let audit_resolved = conn.find_or_create_blob(audit_blob.blob).await?;
                // Pin the exact policy versions this analysis consumed, so the
                // detection is reproducible against them regardless of later edits.
                conn.record_detection_policy_versions(detection_id, &policy_version_ids)
                    .await?;
                // The intermediate is likewise a blob-ref on the detection.
                let (resolved_blob, intermediate_expires_at) = match intermediates_blob {
                    Some(staged) => (
                        Some(conn.find_or_create_blob(staged.blob).await?),
                        Some(staged.expires_at.into()),
                    ),
                    None => (None, None),
                };
                let intermediate_blob_id = resolved_blob.as_ref().map(|blob| blob.id);
                let finalized = conn
                    .finalize_detection(
                        detection.id,
                        claim_token,
                        UpdateWorkspaceDetection {
                            intermediate_blob_id: Some(intermediate_blob_id),
                            intermediate_expires_at: Some(intermediate_expires_at),
                            audit_blob_id: Some(Some(audit_resolved.id)),
                            audit_expires_at: Some(Some(audit_blob.expires_at.into())),
                            ..Default::default()
                        },
                    )
                    .await?;
                if !finalized {
                    // Abort the file inserts: the detection is no longer ours to
                    // finalize. `RollbackTransaction` unwinds the writes
                    // without being a real error; it is matched below.
                    return Err(PgError::Query(DieselError::RollbackTransaction));
                }
                conn.insert_outbox(outbox_row).await?;
                // The resolved storage paths, so an object staged for content that
                // deduplicated onto an existing blob can be reclaimed after commit.
                let intermediate_path = resolved_blob.map(|blob| blob.storage_path);
                Ok::<_, PgError>((audit_resolved.storage_path, intermediate_path))
            })
            .await;

        match finalized {
            Ok((audit_path, intermediate_path)) => {
                // A base audit or intermediate whose content deduplicated onto an
                // existing blob is pointed at that blob's stored object, orphaning
                // the object staged for it. Reclaim the redundant staged objects.
                if audit_path != staged_audit.blob.storage_path {
                    self.writer.discard_staged(&[&staged_audit]).await;
                }
                if let Some(staged) = &staged_intermediates
                    && intermediate_path.as_deref() != Some(staged.blob.storage_path.as_str())
                {
                    self.writer.discard_staged(&[staged]).await;
                }
            }
            Err(PgError::Query(DieselError::RollbackTransaction)) => {
                self.writer
                    .discard_staged(&staged_outputs(
                        &staged_audit,
                        staged_intermediates.as_ref(),
                    ))
                    .await;
                tracing::warn!(target: TRACING_TARGET, detection_id = %detection.id, "Claim went stale before finalize; another worker owns the detection");
                return Ok(());
            }
            Err(err) => {
                // The transaction rolled back, so the file rows never committed;
                // reclaim their objects before surfacing the failure.
                self.writer
                    .discard_staged(&staged_outputs(
                        &staged_audit,
                        staged_intermediates.as_ref(),
                    ))
                    .await;
                return Err(err.into());
            }
        }

        tracing::info!(target: TRACING_TARGET, detection_id = %detection.id, "Detection complete");
        broadcast_status(&self.infra, detection.id, DetectionStatus::Complete).await;

        Ok(())
    }
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

/// The objects a detection staged: its audit, and its intermediates when the
/// analysis produced any.
fn staged_outputs<'a>(
    audit: &'a StagedBlob,
    intermediates: Option<&'a StagedBlob>,
) -> Vec<&'a StagedBlob> {
    iter::once(audit).chain(intermediates).collect()
}

/// Builds the [`RequestContext`] for one detect run.
///
/// Recognition is entirely engine-owned (the built-in pattern set plus the
/// deployment's NER/LLM lineups always run). The document context is the detect
/// request's own. `raster_mode` is the workspace's page-rasterisation policy (always
/// render vs. auto), carried in the codec params since it is server-derived, not
/// caller-set. The engine records the context and codec params on the audit so
/// redaction re-decodes and re-compiles against exactly what detection used.
/// Deduplication and calibration are engine-owned defaults; the label catalog is
/// derived from the run's policies at detect time. No key is set: the keyed
/// operators (`HmacHash`, `Encrypt`) only run at redaction, with the key the
/// redact request supplies.
fn request_context(
    document_context: Option<DocumentContext>,
    raster_mode: RasterMode,
) -> RequestContext {
    RequestContext::new()
        .with_context(document_context.unwrap_or_default())
        .with_codec(CodecParams::new().with_raster_mode(raster_mode))
}
