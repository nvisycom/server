//! Repository for a detection's policy-version pins.
//!
//! The pins record the exact policy versions a detection's analysis ran against,
//! captured in the transaction that commits the analysis. They make a detection
//! reproducible against the config that produced it.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::DetectionPolicyVersion;
use crate::{Error, PgConnection, Result, schema};

/// Repository for the detection policy-version pin table.
pub trait DetectionPolicyVersionRepository {
    /// Records the policy versions a detection's analysis ran against.
    ///
    /// Run inside the transaction that commits the analysis so the pins commit
    /// with it. A repeat of the same `(detection, version)` is ignored.
    fn record_detection_policy_versions(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
        version_ids: &[Uuid],
    ) -> impl Future<Output = Result<()>> + Send;

    /// Lists the policy-version ids a detection pinned.
    fn list_detection_policy_versions(
        &mut self,
        detection_id: Uuid,
    ) -> impl Future<Output = Result<Vec<Uuid>>> + Send;
}

impl DetectionPolicyVersionRepository for PgConnection {
    async fn record_detection_policy_versions(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
        version_ids: &[Uuid],
    ) -> Result<()> {
        use schema::workspace_detection_policy_versions;

        if version_ids.is_empty() {
            return Ok(());
        }

        let rows: Vec<DetectionPolicyVersion> = dedup(version_ids)
            .into_iter()
            .map(|policy_version_id| DetectionPolicyVersion {
                detection_id,
                policy_version_id,
                workspace_id,
            })
            .collect();

        diesel::insert_into(workspace_detection_policy_versions::table)
            .values(&rows)
            .on_conflict_do_nothing()
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn list_detection_policy_versions(&mut self, detection_id: Uuid) -> Result<Vec<Uuid>> {
        use schema::workspace_detection_policy_versions::{self, dsl};

        workspace_detection_policy_versions::table
            .filter(dsl::detection_id.eq(detection_id))
            .select(dsl::policy_version_id)
            .load(self)
            .await
            .map_err(Error::from)
    }
}

/// Deduplicates ids, preserving first-seen order.
fn dedup(ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::with_capacity(ids.len());
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

#[cfg(test)]
mod tests {
    use crate::model::{NewWorkspaceDetection, NewWorkspacePolicy};
    use crate::query::{
        DetectionPolicyVersionRepository, WorkspaceDetectionRepository, WorkspacePolicyRepository,
    };
    use crate::test_util::TestDatabase;

    #[tokio::test]
    async fn records_dedups_and_lists_pinned_versions() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.pipeline_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        let first = conn
            .create_workspace_policy(
                NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id),
                serde_json::json!({ "v": 1 }),
                None,
            )
            .await?;
        let second = conn
            .create_workspace_policy(
                NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id),
                serde_json::json!({ "v": 2 }),
                None,
            )
            .await?;

        // A repeated version id is deduplicated.
        conn.record_detection_policy_versions(
            seeded.workspace_id,
            detection.id,
            &[first.version.id, second.version.id, first.version.id],
        )
        .await?;

        let mut pinned = conn.list_detection_policy_versions(detection.id).await?;
        pinned.sort();
        let mut expected = vec![first.version.id, second.version.id];
        expected.sort();
        assert_eq!(pinned, expected);

        // Recording again is idempotent (no duplicate-key error).
        conn.record_detection_policy_versions(
            seeded.workspace_id,
            detection.id,
            &[first.version.id],
        )
        .await?;
        assert_eq!(
            conn.list_detection_policy_versions(detection.id)
                .await?
                .len(),
            2
        );
        Ok(())
    }

    #[tokio::test]
    async fn recording_an_empty_set_is_a_noop() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.pipeline_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;

        conn.record_detection_policy_versions(seeded.workspace_id, detection.id, &[])
            .await?;
        assert!(
            conn.list_detection_policy_versions(detection.id)
                .await?
                .is_empty()
        );
        Ok(())
    }
}
