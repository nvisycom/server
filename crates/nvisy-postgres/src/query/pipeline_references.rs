//! Repository for a pipeline's policy references.
//!
//! References live in the `workspace_pipeline_policies` join table rather than
//! the pipeline's JSON definition, so foreign keys enforce that every referenced
//! policy exists in the pipeline's workspace. The `replace_*` operation is
//! delete-then-insert and expects to run inside a caller-owned transaction.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::PipelinePolicy;
use crate::types::{Handle, PolicyKind};
use crate::{Error, PgConnection, Result, schema};

/// Repository for pipeline reference join tables.
pub trait PipelineReferenceRepository {
    /// Replaces a pipeline's policy references with the given set.
    ///
    /// Deletes existing references then inserts the new ones. Run inside a
    /// transaction with the pipeline write so the two stay consistent.
    fn replace_workspace_pipeline_policies(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        policy_ids: &[Uuid],
    ) -> impl Future<Output = Result<()>> + Send;

    /// Lists the ids of the policies a pipeline references.
    ///
    /// Used by the run path to resolve each referenced policy to its record for
    /// the engine; the API-facing read path uses [`Self::list_pipeline_policy_slugs`].
    fn list_pipeline_policy_ids(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = Result<Vec<Uuid>>> + Send;

    /// Lists the slugs of the policies a pipeline references.
    fn list_pipeline_policy_slugs(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = Result<Vec<Handle>>> + Send;

    /// Resolves policy slugs to their ids within a workspace, preserving order.
    ///
    /// Returns `None` if any slug does not match a live policy in the workspace,
    /// so the caller can reject the whole set rather than silently dropping an
    /// unknown reference.
    fn resolve_policy_slugs(
        &mut self,
        workspace_id: Uuid,
        slugs: &[Handle],
    ) -> impl Future<Output = Result<Option<Vec<Uuid>>>> + Send;
}

impl PipelineReferenceRepository for PgConnection {
    async fn replace_workspace_pipeline_policies(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        policy_ids: &[Uuid],
    ) -> Result<()> {
        use schema::workspace_pipeline_policies::{self, dsl};

        diesel::delete(workspace_pipeline_policies::table.filter(dsl::pipeline_id.eq(pipeline_id)))
            .execute(self)
            .await
            .map_err(Error::from)?;

        if !policy_ids.is_empty() {
            let rows: Vec<PipelinePolicy> = dedup(policy_ids)
                .into_iter()
                .map(|policy_id| PipelinePolicy {
                    workspace_id,
                    pipeline_id,
                    policy_id,
                })
                .collect();

            diesel::insert_into(workspace_pipeline_policies::table)
                .values(&rows)
                .execute(self)
                .await
                .map_err(Error::from)?;
        }

        Ok(())
    }

    async fn list_pipeline_policy_ids(&mut self, pipeline_id: Uuid) -> Result<Vec<Uuid>> {
        use schema::{workspace_pipeline_policies, workspace_policies};

        let ids = workspace_pipeline_policies::table
            .inner_join(
                workspace_policies::table
                    .on(workspace_policies::id.eq(workspace_pipeline_policies::policy_id)),
            )
            .filter(workspace_pipeline_policies::pipeline_id.eq(pipeline_id))
            .filter(workspace_policies::deleted_at.is_null())
            .select(workspace_pipeline_policies::policy_id)
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(ids)
    }

    async fn list_pipeline_policy_slugs(&mut self, pipeline_id: Uuid) -> Result<Vec<Handle>> {
        use schema::{workspace_pipeline_policies, workspace_policies};

        // Join to the parent so soft-deleted policies (deleted_at set, join row
        // still present since CASCADE only fires on hard delete) are excluded.
        let slugs = workspace_pipeline_policies::table
            .inner_join(
                workspace_policies::table
                    .on(workspace_policies::id.eq(workspace_pipeline_policies::policy_id)),
            )
            .filter(workspace_pipeline_policies::pipeline_id.eq(pipeline_id))
            .filter(workspace_policies::deleted_at.is_null())
            .select(workspace_policies::slug)
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(slugs)
    }

    async fn resolve_policy_slugs(
        &mut self,
        workspace_id: Uuid,
        slugs: &[Handle],
    ) -> Result<Option<Vec<Uuid>>> {
        use schema::workspace_policies::{self, dsl};

        if slugs.is_empty() {
            return Ok(Some(Vec::new()));
        }

        // One-shot policies are not attachable to a pipeline: a pipeline references
        // authored policies only. Excluding them here means a one-shot slug resolves
        // as unknown, so attachment rejects it.
        let wanted: Vec<String> = slugs.iter().map(|slug| slug.as_str().to_owned()).collect();
        let found: Vec<(Handle, Uuid)> = workspace_policies::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::kind.eq(PolicyKind::Authored))
            .filter(dsl::slug.eq_any(&wanted))
            .select((dsl::slug, dsl::id))
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(map_slugs_to_ids(slugs, found))
    }
}

/// Maps the requested slugs to ids in request order, returning `None` if any
/// requested slug is missing from the resolved set.
fn map_slugs_to_ids(requested: &[Handle], found: Vec<(Handle, Uuid)>) -> Option<Vec<Uuid>> {
    let by_slug: std::collections::HashMap<Handle, Uuid> = found.into_iter().collect();
    requested
        .iter()
        .map(|slug| by_slug.get(slug).copied())
        .collect()
}

/// Deduplicates ids, preserving first-seen order.
fn dedup(ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::with_capacity(ids.len());
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::model::{NewWorkspacePipeline, NewWorkspacePolicy};
    use crate::query::{
        PipelineReferenceRepository, WorkspacePipelineRepository, WorkspacePolicyRepository,
    };
    use crate::test_util::TestDatabase;
    use crate::types::{Handle, PolicyKind};

    /// Seeds a pipeline plus `count` policies, returning `(workspace_id,
    /// pipeline_id, policy_ids)`.
    async fn seed(db: &TestDatabase, count: usize) -> anyhow::Result<(Uuid, Uuid, Vec<Uuid>)> {
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let pipeline = conn
            .create_workspace_pipeline(NewWorkspacePipeline::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let mut policy_ids = Vec::new();
        for _ in 0..count {
            let policy = conn
                .create_test_policy(NewWorkspacePolicy::test(
                    seeded.workspace_id,
                    seeded.account_id,
                ))
                .await?;
            policy_ids.push(policy.id);
        }
        Ok((seeded.workspace_id, pipeline.id, policy_ids))
    }

    #[tokio::test]
    async fn replace_sets_dedups_and_clears_references() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (workspace_id, pipeline_id, policies) = seed(&db, 2).await?;
        let mut conn = db.client.get_connection().await?;

        // Replace with a set that names one policy twice: it is deduplicated.
        conn.replace_workspace_pipeline_policies(
            workspace_id,
            pipeline_id,
            &[policies[0], policies[1], policies[0]],
        )
        .await?;
        let mut ids = conn.list_pipeline_policy_ids(pipeline_id).await?;
        ids.sort();
        let mut expected = vec![policies[0], policies[1]];
        expected.sort();
        assert_eq!(ids, expected);

        // Replacing with a smaller set overwrites (delete-then-insert).
        conn.replace_workspace_pipeline_policies(workspace_id, pipeline_id, &[policies[1]])
            .await?;
        assert_eq!(
            conn.list_pipeline_policy_ids(pipeline_id).await?,
            vec![policies[1]]
        );

        // Replacing with an empty set clears all references.
        conn.replace_workspace_pipeline_policies(workspace_id, pipeline_id, &[])
            .await?;
        assert!(conn.list_pipeline_policy_ids(pipeline_id).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn listings_exclude_soft_deleted_policies() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (workspace_id, pipeline_id, policies) = seed(&db, 2).await?;
        let mut conn = db.client.get_connection().await?;

        conn.replace_workspace_pipeline_policies(workspace_id, pipeline_id, &policies)
            .await?;
        assert_eq!(conn.list_pipeline_policy_ids(pipeline_id).await?.len(), 2);
        assert_eq!(conn.list_pipeline_policy_slugs(pipeline_id).await?.len(), 2);

        // Soft-deleting a referenced policy drops it from both listings (the join
        // row remains, but the parent is filtered on `deleted_at`).
        conn.delete_workspace_policy(policies[0]).await?;
        assert_eq!(
            conn.list_pipeline_policy_ids(pipeline_id).await?,
            vec![policies[1]]
        );
        assert_eq!(conn.list_pipeline_policy_slugs(pipeline_id).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn resolve_policy_slugs_preserves_order_and_rejects_unknown() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let alpha = conn
            .create_test_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let bravo = conn
            .create_test_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // Resolution preserves request order, not storage order.
        let resolved = conn
            .resolve_policy_slugs(
                seeded.workspace_id,
                &[bravo.slug.clone(), alpha.slug.clone()],
            )
            .await?;
        assert_eq!(resolved, Some(vec![bravo.id, alpha.id]));

        // An empty request resolves to an empty vec (not `None`).
        assert_eq!(
            conn.resolve_policy_slugs(seeded.workspace_id, &[]).await?,
            Some(Vec::new())
        );

        // If any slug is unknown, the whole set is rejected with `None`.
        let unknown = Handle::test();
        assert_eq!(
            conn.resolve_policy_slugs(seeded.workspace_id, &[alpha.slug.clone(), unknown])
                .await?,
            None
        );

        // A slug that exists only in another workspace does not resolve here.
        assert_eq!(
            conn.resolve_policy_slugs(Uuid::now_v7(), std::slice::from_ref(&alpha.slug))
                .await?,
            None
        );
        Ok(())
    }

    #[tokio::test]
    async fn resolve_policy_slugs_ignores_oneshot_policies() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let mut new_oneshot = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        new_oneshot.kind = PolicyKind::Oneshot;
        new_oneshot.content_hash = Some(vec![3, 3, 3]);
        let oneshot = conn
            .find_or_create_oneshot_policy(
                new_oneshot,
                vec![3, 3, 3],
                serde_json::json!({"v":1}),
                None,
            )
            .await?;
        let slug = oneshot.policy.policy.slug.clone();

        // A one-shot policy's slug does not resolve for pipeline attachment, so the
        // set is rejected as if the slug were unknown.
        assert_eq!(
            conn.resolve_policy_slugs(seeded.workspace_id, std::slice::from_ref(&slug))
                .await?,
            None
        );

        // Once promoted (authored, hash cleared), it resolves.
        conn.update_workspace_policy(
            oneshot.policy.policy.id,
            crate::model::UpdateWorkspacePolicy {
                kind: Some(PolicyKind::Authored),
                content_hash: Some(None),
                ..Default::default()
            },
        )
        .await?;
        assert_eq!(
            conn.resolve_policy_slugs(seeded.workspace_id, std::slice::from_ref(&slug))
                .await?,
            Some(vec![oneshot.policy.policy.id])
        );
        Ok(())
    }
}
