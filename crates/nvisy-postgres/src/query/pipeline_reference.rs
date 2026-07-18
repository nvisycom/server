//! Repository for a pipeline's policy and context references.
//!
//! References live in join tables (`workspace_pipeline_policies`, `workspace_pipeline_contexts`)
//! rather than the pipeline's JSON definition, so foreign keys enforce that
//! every referenced policy/context exists in the pipeline's workspace. The
//! `replace_*` operations are delete-then-insert and expect to run inside a
//! caller-owned transaction.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{PipelineContext, PipelinePolicy};
use crate::types::Slug;
use crate::{PgConnection, PgError, PgResult, schema};

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
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Replaces a pipeline's context references with the given set.
    fn replace_workspace_pipeline_contexts(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        context_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists the ids of the policies a pipeline references.
    ///
    /// Used by the run path to resolve each referenced policy to its record for
    /// the engine; the API-facing read path uses [`Self::list_pipeline_policy_slugs`].
    fn list_pipeline_policy_ids(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<Uuid>>> + Send;

    /// Lists the ids of the contexts a pipeline references.
    fn list_pipeline_context_ids(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<Uuid>>> + Send;

    /// Lists the slugs of the policies a pipeline references.
    fn list_pipeline_policy_slugs(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<Slug>>> + Send;

    /// Lists the slugs of the contexts a pipeline references.
    fn list_pipeline_context_slugs(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<Slug>>> + Send;

    /// Resolves policy slugs to their ids within a workspace, preserving order.
    ///
    /// Returns `None` if any slug does not match a live policy in the workspace,
    /// so the caller can reject the whole set rather than silently dropping an
    /// unknown reference.
    fn resolve_policy_slugs(
        &mut self,
        workspace_id: Uuid,
        slugs: &[Slug],
    ) -> impl Future<Output = PgResult<Option<Vec<Uuid>>>> + Send;

    /// Resolves context slugs to their ids within a workspace, preserving order.
    fn resolve_context_slugs(
        &mut self,
        workspace_id: Uuid,
        slugs: &[Slug],
    ) -> impl Future<Output = PgResult<Option<Vec<Uuid>>>> + Send;
}

impl PipelineReferenceRepository for PgConnection {
    async fn replace_workspace_pipeline_policies(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        policy_ids: &[Uuid],
    ) -> PgResult<()> {
        use schema::workspace_pipeline_policies::{self, dsl};

        diesel::delete(workspace_pipeline_policies::table.filter(dsl::pipeline_id.eq(pipeline_id)))
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

            diesel::insert_into(workspace_pipeline_policies::table)
                .values(&rows)
                .execute(self)
                .await
                .map_err(PgError::from)?;
        }

        Ok(())
    }

    async fn replace_workspace_pipeline_contexts(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
        context_ids: &[Uuid],
    ) -> PgResult<()> {
        use schema::workspace_pipeline_contexts::{self, dsl};

        diesel::delete(workspace_pipeline_contexts::table.filter(dsl::pipeline_id.eq(pipeline_id)))
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

            diesel::insert_into(workspace_pipeline_contexts::table)
                .values(&rows)
                .execute(self)
                .await
                .map_err(PgError::from)?;
        }

        Ok(())
    }

    async fn list_pipeline_policy_ids(&mut self, pipeline_id: Uuid) -> PgResult<Vec<Uuid>> {
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
            .map_err(PgError::from)?;

        Ok(ids)
    }

    async fn list_pipeline_context_ids(&mut self, pipeline_id: Uuid) -> PgResult<Vec<Uuid>> {
        use schema::{workspace_contexts, workspace_pipeline_contexts};

        let ids = workspace_pipeline_contexts::table
            .inner_join(
                workspace_contexts::table
                    .on(workspace_contexts::id.eq(workspace_pipeline_contexts::context_id)),
            )
            .filter(workspace_pipeline_contexts::pipeline_id.eq(pipeline_id))
            .filter(workspace_contexts::deleted_at.is_null())
            .select(workspace_pipeline_contexts::context_id)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(ids)
    }

    async fn list_pipeline_policy_slugs(&mut self, pipeline_id: Uuid) -> PgResult<Vec<Slug>> {
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
            .map_err(PgError::from)?;

        Ok(slugs)
    }

    async fn list_pipeline_context_slugs(&mut self, pipeline_id: Uuid) -> PgResult<Vec<Slug>> {
        use schema::{workspace_contexts, workspace_pipeline_contexts};

        let slugs = workspace_pipeline_contexts::table
            .inner_join(
                workspace_contexts::table
                    .on(workspace_contexts::id.eq(workspace_pipeline_contexts::context_id)),
            )
            .filter(workspace_pipeline_contexts::pipeline_id.eq(pipeline_id))
            .filter(workspace_contexts::deleted_at.is_null())
            .select(workspace_contexts::slug)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(slugs)
    }

    async fn resolve_policy_slugs(
        &mut self,
        workspace_id: Uuid,
        slugs: &[Slug],
    ) -> PgResult<Option<Vec<Uuid>>> {
        use schema::workspace_policies::{self, dsl};

        if slugs.is_empty() {
            return Ok(Some(Vec::new()));
        }

        let wanted: Vec<String> = slugs.iter().map(|slug| slug.as_str().to_owned()).collect();
        let found: Vec<(Slug, Uuid)> = workspace_policies::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::slug.eq_any(&wanted))
            .select((dsl::slug, dsl::id))
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(map_slugs_to_ids(slugs, found))
    }

    async fn resolve_context_slugs(
        &mut self,
        workspace_id: Uuid,
        slugs: &[Slug],
    ) -> PgResult<Option<Vec<Uuid>>> {
        use schema::workspace_contexts::{self, dsl};

        if slugs.is_empty() {
            return Ok(Some(Vec::new()));
        }

        let wanted: Vec<String> = slugs.iter().map(|slug| slug.as_str().to_owned()).collect();
        let found: Vec<(Slug, Uuid)> = workspace_contexts::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::slug.eq_any(&wanted))
            .select((dsl::slug, dsl::id))
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(map_slugs_to_ids(slugs, found))
    }
}

/// Maps the requested slugs to ids in request order, returning `None` if any
/// requested slug is missing from the resolved set.
fn map_slugs_to_ids(requested: &[Slug], found: Vec<(Slug, Uuid)>) -> Option<Vec<Uuid>> {
    let by_slug: std::collections::HashMap<Slug, Uuid> = found.into_iter().collect();
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
