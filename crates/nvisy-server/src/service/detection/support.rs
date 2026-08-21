//! Shared detection helpers used by both the create-run handler and the worker.

use elide_pipeline::policy::PolicyDefinition;
use nvisy_postgres::model::{NewWorkspacePipelineRunUsage, UpdateWorkspacePipelineRun};
use nvisy_postgres::query::{
    PipelineReferenceRepository, WorkspacePipelineRunRepository, WorkspacePolicyRepository,
};
use nvisy_postgres::types::{Handle, Json, PipelineRunStatus, RunMetadata};
use uuid::Uuid;

use super::service::DetectionQueue;
use crate::extract::SecurityContext;
use crate::handler::Result;
use crate::service::{CryptoService, EventEmitter, EventOrigin, PipelineRunRef, WorkspaceEvent};

/// Tracing target for shared detection operations.
const TRACING_TARGET: &str = "nvisy_server::service::detection";

/// A run's inference usage, extracted from the engine's [`Audit`]: the per-model
/// token rows for the usage table and the full report as JSON for the metadata
/// blob. Absent when the run used no model-based recognizers.
///
/// [`Audit`]: elide_pipeline::Audit
pub(crate) struct RunUsage {
    /// One row per distinct model, tokens summed across the recognizers that
    /// shared it. Empty when no recognizer used a model.
    pub per_model: Vec<NewWorkspacePipelineRunUsage>,
    /// The full per-recognizer usage report, serialized for `RunMetadata.usage`.
    pub report: serde_json::Value,
}

/// Extracts a run's inference usage from the analysis, or `None` when the report
/// is empty (a purely deterministic run spends no tokens).
///
/// Recognizers are grouped by `(model, version)` and their token counts summed —
/// `input`, `output`, and `total` independently, since a provider may report a
/// `total` that is not `input + output` (cached/reasoning tokens). Durations are
/// summed into milliseconds. The whole report is also kept as JSON for drill-down.
pub(crate) fn extract_run_usage(
    run_id: Uuid,
    analyzed: &elide_pipeline::Audit,
) -> Option<RunUsage> {
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
        .map(|((model, version), acc)| NewWorkspacePipelineRunUsage {
            run_id,
            model,
            version,
            input_tokens: acc.input.map(to_i64),
            output_tokens: acc.output.map(to_i64),
            total_tokens: acc.total.map(to_i64),
            duration_ms: acc.duration_ms.try_into().unwrap_or(i64::MAX),
        })
        .collect();

    let report = serde_json::to_value(usage).unwrap_or(serde_json::Value::Null);

    Some(RunUsage { per_model, report })
}

/// Clamps an upstream `u64` token count into the `i64` column; token counts are
/// well within range in practice, so saturating beats wrapping if one ever isn't.
fn to_i64(value: u64) -> i64 {
    value.try_into().unwrap_or(i64::MAX)
}

/// Identifies the run to fail and how, so [`fail_run`] takes one bundle rather
/// than a long positional list.
pub(crate) struct FailRun<'a> {
    /// The workspace the run belongs to.
    pub workspace_id: Uuid,
    /// The run to fail.
    pub run_id: Uuid,
    /// Slug of the run's pipeline, for the emitted event.
    pub pipeline_slug: Handle,
    /// The account that triggered the run (the failure's actor and notify target).
    pub triggered_by: Uuid,
    /// Human-readable failure reason, stored in the run's metadata.
    pub reason: &'a str,
    /// The worker's claim timestamp, guarding the transition; `None` on the
    /// handler path, which has no claim to fence.
    pub claim: Option<jiff::Timestamp>,
}

/// Marks a run `Failed` (best effort), recording the reason in its metadata,
/// broadcasting the terminal status for SSE watchers, and emitting the
/// `PipelineRunFailed` event (activity log, webhook, and owner notification).
///
/// Shared by the create-run handler (enqueue failed) and the worker (analysis
/// failed) so a failure takes the same steps on every path.
///
/// `params.claim` fences the worker path: when `Some(claimed_at)`, the run is
/// failed only while that claim still holds (still `Analyzing`, `claimed_at`
/// unchanged), and the broadcast/event fire only if it did — so a worker whose
/// lease expired mid-analysis cannot fail, or announce the failure of, a run
/// another worker now owns. The handler passes `None`: it fails the run it just
/// created, with no claim to guard.
pub(crate) async fn fail_run(
    conn: &mut nvisy_postgres::PgConn,
    detection: &DetectionQueue,
    params: FailRun<'_>,
) {
    let FailRun {
        workspace_id,
        run_id,
        pipeline_slug,
        triggered_by,
        reason,
        claim,
    } = params;

    let metadata = RunMetadata {
        error: Some(reason.to_owned()),
        ..Default::default()
    };
    let update = UpdateWorkspacePipelineRun {
        status: Some(PipelineRunStatus::Failed),
        metadata: Some(Json::encode(&metadata)),
        completed_at: Some(Some(jiff::Timestamp::now().into())),
        ..Default::default()
    };

    match claim {
        // Worker path: guard on the claim. A stale claim fails nothing and stays
        // silent — the new owner drives the run to its own outcome.
        Some(claimed_at) => match conn.finalize_failed_run(run_id, claimed_at, update).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(target: TRACING_TARGET, %run_id, "Claim went stale before failure; another worker owns the run");
                return;
            }
            Err(err) => {
                tracing::warn!(target: TRACING_TARGET, error = %err, %run_id, "Failed to mark run failed");
                return;
            }
        },
        // Handler path: no claim to guard.
        None => {
            if let Err(err) = conn.update_workspace_pipeline_run(run_id, update).await {
                tracing::warn!(target: TRACING_TARGET, error = %err, %run_id, "Failed to mark run failed");
            }
        }
    }

    detection
        .broadcast_status(run_id, PipelineRunStatus::Failed)
        .await;

    if let Err(err) = conn
        .emit_event(
            EventOrigin {
                workspace_id,
                account_id: triggered_by,
                security: &SecurityContext::default(),
            },
            WorkspaceEvent::PipelineRunFailed {
                run: PipelineRunRef {
                    run_id,
                    pipeline_slug,
                },
                input_file_name: None,
                error: Some(reason.to_owned()),
                notify: triggered_by,
            },
        )
        .await
    {
        tracing::warn!(target: TRACING_TARGET, error = %err, %run_id, "Failed to record run-failed event");
    }
}

/// Resolves a pipeline's live policy references into decrypted engine policies.
pub(crate) async fn resolve_policies(
    conn: &mut nvisy_postgres::PgConn,
    crypto: &CryptoService,
    workspace_id: Uuid,
    pipeline_id: Uuid,
) -> Result<Vec<PolicyDefinition>> {
    let ids = conn.list_pipeline_policy_ids(pipeline_id).await?;
    let mut policies = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(model) = conn.find_policy_in_workspace(workspace_id, id).await? {
            policies
                .push(crypto.decrypt_json::<PolicyDefinition>(workspace_id, &model.definition)?);
        }
    }
    Ok(policies)
}
