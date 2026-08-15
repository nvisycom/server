//! Shared detection helpers used by both the create-run handler and the worker.

use elide_pipeline::policy::PolicyDefinition;
use nvisy_postgres::model::UpdateWorkspacePipelineRun;
use nvisy_postgres::query::{
    PipelineReferenceRepository, WorkspacePipelineRunRepository, WorkspacePolicyRepository,
};
use nvisy_postgres::types::{Json, PipelineRunStatus, RunMetadata};
use uuid::Uuid;

use super::service::DetectionQueue;
use crate::handler::Result;
use crate::service::{CryptoService, WebhookEmitter};

/// Tracing target for shared detection operations.
const TRACING_TARGET: &str = "nvisy_server::service::detection";

/// Marks a run `Failed` (best effort), recording `reason` in its metadata,
/// broadcasting the terminal status for SSE watchers, and emitting the
/// `pipeline:run.failed` webhook event.
///
/// Shared by the create-run handler (enqueue failed) and the worker (analysis
/// failed) so a failure takes the same three steps on every path.
pub(crate) async fn fail_run(
    conn: &mut nvisy_postgres::PgConn,
    detection: &DetectionQueue,
    webhook_emitter: &WebhookEmitter,
    workspace_id: Uuid,
    run_id: Uuid,
    triggered_by: Uuid,
    reason: &str,
) {
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
    if let Err(err) = conn.update_workspace_pipeline_run(run_id, update).await {
        tracing::warn!(target: TRACING_TARGET, error = %err, %run_id, "Failed to mark run failed");
    }

    detection
        .broadcast_status(run_id, PipelineRunStatus::Failed)
        .await;

    if let Err(err) = webhook_emitter
        .emit_pipeline_run_failed(workspace_id, run_id, Some(triggered_by), None)
        .await
    {
        tracing::warn!(
            target: TRACING_TARGET,
            error = %err,
            %run_id,
            "Failed to emit pipeline:run.failed webhook event"
        );
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
