//! Repository for a pipeline's policy and context references.
//!
//! References live in join tables (`pipeline_policies`, `pipeline_contexts`)
//! rather than the pipeline's JSON definition, so foreign keys enforce that
//! every referenced policy/context exists in the pipeline's workspace. The
//! `replace_*` operations are delete-then-insert and expect to run inside a
//! caller-owned transaction.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{PipelineContext, PipelinePolicy};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for pipeline reference join tables.
pub trait PipelineReferenceRepository {
    /// Replaces a pipeline's policy references with the given set.
    ///
    /// Deletes existing references then inserts the new ones. Run inside a
    /// transaction with the pipeline write so the two stay consistent.
    fn replace_pipeline_policies(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        policy_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Replaces a pipeline's context references with the given set.
    fn replace_pipeline_contexts(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        context_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists the policy ids a pipeline references.
    fn list_pipeline_policy_ids(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<Uuid>>> + Send;

    /// Lists the context ids a pipeline references.
    fn list_pipeline_context_ids(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<Uuid>>> + Send;
}

impl PipelineReferenceRepository for PgConnection {
    async fn replace_pipeline_policies(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        policy_ids: &[Uuid],
    ) -> PgResult<()> {
        use schema::pipeline_policies::{self, dsl};

        diesel::delete(pipeline_policies::table.filter(dsl::pipeline_id.eq(pipeline_id)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        if !policy_ids.is_empty() {
            let rows: Vec<PipelinePolicy> = dedup(policy_ids)
                .into_iter()
                .map(|policy_id| PipelinePolicy {
                    workspace_id,
                    pipeline_id,
                    policy_id,
                })
                .collect();

            diesel::insert_into(pipeline_policies::table)
                .values(&rows)
                .execute(self)
                .await
                .map_err(PgError::from)?;
        }

        Ok(())
    }

    async fn replace_pipeline_contexts(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        context_ids: &[Uuid],
    ) -> PgResult<()> {
        use schema::pipeline_contexts::{self, dsl};

        diesel::delete(pipeline_contexts::table.filter(dsl::pipeline_id.eq(pipeline_id)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        if !context_ids.is_empty() {
            let rows: Vec<PipelineContext> = dedup(context_ids)
                .into_iter()
                .map(|context_id| PipelineContext {
                    workspace_id,
                    pipeline_id,
                    context_id,
                })
                .collect();

            diesel::insert_into(pipeline_contexts::table)
                .values(&rows)
                .execute(self)
                .await
                .map_err(PgError::from)?;
        }

        Ok(())
    }

    async fn list_pipeline_policy_ids(&mut self, pipeline_id: Uuid) -> PgResult<Vec<Uuid>> {
        use schema::{pipeline_policies, workspace_policies};

        // Join to the parent so soft-deleted policies (deleted_at set, join row
        // still present since CASCADE only fires on hard delete) are excluded.
        let ids = pipeline_policies::table
            .inner_join(
                workspace_policies::table
                    .on(workspace_policies::id.eq(pipeline_policies::policy_id)),
            )
            .filter(pipeline_policies::pipeline_id.eq(pipeline_id))
            .filter(workspace_policies::deleted_at.is_null())
            .select(pipeline_policies::policy_id)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(ids)
    }

    async fn list_pipeline_context_ids(&mut self, pipeline_id: Uuid) -> PgResult<Vec<Uuid>> {
        use schema::{pipeline_contexts, workspace_contexts};

        let ids = pipeline_contexts::table
            .inner_join(
                workspace_contexts::table
                    .on(workspace_contexts::id.eq(pipeline_contexts::context_id)),
            )
            .filter(pipeline_contexts::pipeline_id.eq(pipeline_id))
            .filter(workspace_contexts::deleted_at.is_null())
            .select(pipeline_contexts::context_id)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(ids)
    }
}

/// Deduplicates ids, preserving first-seen order.
fn dedup(ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::with_capacity(ids.len());
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}
