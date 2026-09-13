//! Detection worker helpers: usage extraction, policy resolution for a run, and
//! the shared failure path.

use elide_pipeline::governance::policy::Policy;
use nvisy_postgres::model::{NewWorkspaceDetectionUsage, UpdateWorkspaceDetection};
use nvisy_postgres::query::{
    PipelineReferenceRepository, WorkspaceDetectionRepository, WorkspacePolicyRepository,
};
use nvisy_postgres::types::{DetectionMetadata, DetectionStatus, Json};
use uuid::Uuid;

use crate::extract::SecurityContext;
use crate::response::{ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{Infra, event};
use crate::worker::detection::broadcast_status;

/// Tracing target for detection worker helpers.
const TRACING_TARGET: &str = "nvisy_server::worker::detection";

/// A detection's inference usage, extracted from the engine's [`Audit`]: the
/// per-model token rows for the usage table and the full report as JSON for the
/// metadata blob. Absent when the detection used no model-based recognizers.
///
/// [`Audit`]: elide_pipeline::Audit
pub(crate) struct DetectionUsage {
    /// One row per distinct model, tokens summed across the recognizers that
    /// shared it. Empty when no recognizer used a model.
    pub per_model: Vec<NewWorkspaceDetectionUsage>,
    /// The full per-recognizer usage report, serialized for
    /// `DetectionMetadata.usage`.
    pub report: serde_json::Value,
}

/// Extracts a detection's inference usage from the analysis, or `None` when the
/// report is empty (a purely deterministic detection spends no tokens).
///
/// Recognizers are grouped by `(model, version)` and their token counts summed —
/// `input`, `output`, and `total` independently, since a provider may report a
/// `total` that is not `input + output` (cached/reasoning tokens). Durations are
/// summed into milliseconds. The whole report is also kept as JSON for drill-down.
pub(crate) fn extract_detection_usage(
    detection_id: Uuid,
    analyzed: &elide_pipeline::Audit,
) -> Option<DetectionUsage> {
    use std::collections::BTreeMap;

    let usage = &analyzed.usage;
    if usage.is_empty() {
        return None;
    }

    /// Running per-model totals; each token field stays `None` until a recognizer
    /// reports it, so "not reported" is preserved rather than coerced to 0.
    #[derive(Default)]
    struct Acc {
        input: Option<u64>,
        output: Option<u64>,
        total: Option<u64>,
        duration_ms: u128,
    }

    /// Adds an optional reported count into a running optional sum.
    fn add(acc: &mut Option<u64>, reported: Option<u64>) {
        if let Some(v) = reported {
            *acc = Some(acc.unwrap_or(0).saturating_add(v));
        }
    }

    let mut by_model: BTreeMap<(String, Option<String>), Acc> = BTreeMap::new();
    for entry in &usage.entries {
        let Some(model) = &entry.model else { continue };
        let key = (
            model.model.to_string(),
            model.version.as_ref().map(ToString::to_string),
        );
        let acc = by_model.entry(key).or_default();
        add(&mut acc.input, model.tokens.input);
        add(&mut acc.output, model.tokens.output);
        add(&mut acc.total, model.tokens.total);
        acc.duration_ms = acc.duration_ms.saturating_add(entry.duration.as_millis());
    }

    let per_model = by_model
        .into_iter()
        .map(|((model, version), acc)| NewWorkspaceDetectionUsage {
            detection_id,
            model,
            version,
            input_tokens: acc.input.map(to_i64),
            output_tokens: acc.output.map(to_i64),
            total_tokens: acc.total.map(to_i64),
            duration_ms: acc.duration_ms.try_into().unwrap_or(i64::MAX),
        })
        .collect();

    let report = serde_json::to_value(usage).unwrap_or(serde_json::Value::Null);

    Some(DetectionUsage { per_model, report })
}

/// Clamps an upstream `u64` token count into the `i64` column; token counts are
/// well within range in practice, so saturating beats wrapping if one ever isn't.
fn to_i64(value: u64) -> i64 {
    value.try_into().unwrap_or(i64::MAX)
}

/// Identifies the detection to fail and how, so [`fail_detection`] takes one
/// bundle rather than a long positional list.
pub(crate) struct FailDetection<'a> {
    /// The workspace the detection belongs to.
    pub workspace_id: Uuid,
    /// The detection to fail.
    pub detection_id: Uuid,
    /// Id of the detection's pipeline, for the emitted event; `None` for an
    /// ad-hoc detection.
    pub pipeline_id: Option<Uuid>,
    /// The account that triggered the detection (the failure's actor and notify
    /// target).
    pub triggered_by: Uuid,
    /// Human-readable failure reason, stored in the detection's metadata.
    pub reason: &'a str,
    /// The detection's current metadata, so the failure reason is layered onto it
    /// rather than replacing recorded fields such as tags.
    pub metadata: DetectionMetadata,
    /// The worker's claim timestamp, guarding the transition; `None` when there is
    /// no claim to fence (a job that never reached a worker).
    pub claim: Option<jiff::Timestamp>,
}

/// Marks a detection `Failed` (best effort), recording the reason in its
/// metadata, broadcasting the terminal status for SSE watchers, and emitting the
/// `DetectionFailed` event (activity log, webhook, and owner notification).
///
/// Shared by the worker (analysis failed) and the drainer (a dead-lettered job)
/// so a failure takes the same steps on every path.
///
/// `params.claim` fences the worker path: when `Some(claimed_at)`, the detection
/// is failed only while that claim still holds (still `Executing`, `claimed_at`
/// unchanged), and the broadcast/event fire only if it did — so a worker whose
/// lease expired mid-analysis cannot fail, or announce the failure of, a
/// detection another worker now owns. `None` guards on `Pending` instead, for a
/// job that never reached a worker.
pub(crate) async fn fail_detection(
    conn: &mut nvisy_postgres::PgConn,
    infra: &Infra,
    params: FailDetection<'_>,
) -> FailOutcome {
    let FailDetection {
        workspace_id,
        detection_id,
        pipeline_id,
        triggered_by,
        reason,
        mut metadata,
        claim,
    } = params;

    // Layer the failure reason onto the detection's existing metadata so recorded
    // fields (e.g. reviewer tags) survive the failure write.
    metadata.error = Some(reason.to_owned());
    // `status` and `completed_at` are forced by the finalize methods themselves
    // (the terminal transition owns the terminal timestamp), so only the metadata
    // is supplied here.
    let update = UpdateWorkspaceDetection {
        metadata: Some(Json::encode(&metadata)),
        ..Default::default()
    };

    let persisted = match claim {
        // Worker path: guard on the claim. A stale claim fails nothing and stays
        // silent — the new owner drives the detection to its own outcome.
        Some(claimed_at) => conn.fail_detection(detection_id, claimed_at, update).await,
        // No claim: guard on `Pending` so this no-ops if a worker already claimed
        // the detection — the worker then owns the outcome.
        None => conn.fail_pending_detection(detection_id, update).await,
    };
    match persisted {
        Ok(true) => {}
        // The guard didn't match: another owner (or an already-terminal detection)
        // drives the outcome. Nothing to persist, announce, or retry.
        Ok(false) => {
            tracing::warn!(target: TRACING_TARGET, %detection_id, "Detection no longer owned at failure; another owner drives it");
            return FailOutcome::NotOwned;
        }
        // The terminal write itself failed: the detection is still `Executing`
        // with no queued job to reclaim it, so the caller must retry (redeliver)
        // rather than treat the failure as handled.
        Err(err) => {
            tracing::warn!(target: TRACING_TARGET, error = %err, %detection_id, "Failed to persist detection failure; will retry");
            return FailOutcome::PersistFailed;
        }
    }

    broadcast_status(infra, detection_id, DetectionStatus::Failed).await;

    if let Err(err) = conn
        .emit_event(
            event::EventOrigin {
                workspace_id,
                account_id: triggered_by,
                security: &SecurityContext::default(),
            },
            event::WorkspaceEvent::DetectionFailed(event::DetectionFailed {
                detection_id,
                pipeline_id,
                input_document_name: None,
                error: Some(reason.to_owned()),
                notify: triggered_by,
            }),
        )
        .await
    {
        tracing::warn!(target: TRACING_TARGET, error = %err, %detection_id, "Failed to record detection-failed event");
    }

    // The state transition is persisted; a lost event is recoverable from the
    // outbox and does not change the outcome.
    FailOutcome::Failed
}

/// The result of attempting to fail a detection, so a caller driving a work
/// queue can decide whether to redeliver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailOutcome {
    /// The detection was transitioned to `Failed` and its failure announced.
    Failed,
    /// The failure was not applied because the detection is no longer owned by
    /// the caller (another owner, or an already-terminal detection). No retry.
    NotOwned,
    /// The terminal write failed to persist, leaving the detection mid-flight;
    /// the caller should redeliver so the stale lease is reclaimed.
    PersistFailed,
}

/// A policy resolved for a detection run: its definition and the id of the
/// version that definition came from, so the run can pin what it consumed.
pub(crate) struct ResolvedPolicy {
    /// The version the definition was resolved from.
    pub version_id: Uuid,
    /// The policy the engine consumes.
    pub definition: Policy,
}

/// Deserializes a stored policy definition (plaintext JSONB) into an engine
/// [`Policy`], failing the run if the stored body is malformed.
fn parse_definition(version_id: Uuid, definition: serde_json::Value) -> Result<Policy> {
    serde_json::from_value(definition).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Stored policy definition is malformed")
            .with_context(format!("policy_version_id: {version_id}: {err}"))
    })
}

/// Resolves a pipeline's live policy references to their current versions.
///
/// Each resolved policy carries the version id it came from, so the caller can
/// pin the exact versions the detection ran against.
pub(crate) async fn resolve_policies(
    conn: &mut nvisy_postgres::PgConn,
    workspace_id: Uuid,
    pipeline_id: Uuid,
) -> Result<Vec<ResolvedPolicy>> {
    let ids = conn.list_pipeline_policy_ids(pipeline_id).await?;
    let mut policies = Vec::with_capacity(ids.len());
    for id in ids {
        // Every referenced policy must resolve: a policy soft-deleted between
        // listing and lookup would otherwise silently shrink the policy set and
        // under-redact the document. Fail the run instead.
        let found = conn
            .find_policy_with_version(workspace_id, id)
            .await?
            .ok_or_else(|| {
                ErrorKind::InternalServerError
                    .with_message("Referenced policy is no longer available")
                    .with_context(format!("policy_id: {id}"))
            })?;
        let definition = parse_definition(found.version.id, found.version.definition)?;
        policies.push(ResolvedPolicy {
            version_id: found.version.id,
            definition,
        });
    }
    Ok(policies)
}

/// Resolves an explicit list of policy ids to their current versions, for an
/// ad-hoc detection that names its policies directly rather than through a
/// pipeline.
///
/// Each id must resolve to a live policy of any kind (authored or one-shot) in
/// the workspace; an unknown id fails the run. Returns the same
/// [`ResolvedPolicy`] shape as [`resolve_policies`], so the caller pins versions
/// identically.
pub(crate) async fn resolve_policies_by_ids(
    conn: &mut nvisy_postgres::PgConn,
    workspace_id: Uuid,
    policy_ids: &[Uuid],
) -> Result<Vec<ResolvedPolicy>> {
    let mut policies = Vec::with_capacity(policy_ids.len());
    for policy_id in policy_ids {
        let policy = conn
            .find_policy_in_workspace_by_id(workspace_id, *policy_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::InternalServerError
                    .with_message("Referenced policy is no longer available")
                    .with_context(format!("policy_id: {policy_id}"))
            })?;
        let found = conn
            .find_policy_with_version(workspace_id, policy.item.id)
            .await?
            .ok_or_else(|| {
                ErrorKind::InternalServerError
                    .with_message("Referenced policy is no longer available")
                    .with_context(format!("policy_id: {policy_id}"))
            })?;
        let definition = parse_definition(found.version.id, found.version.definition)?;
        policies.push(ResolvedPolicy {
            version_id: found.version.id,
            definition,
        });
    }
    Ok(policies)
}
