//! Pipelines repository for managing workflow definitions.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{NewWorkspacePipeline, UpdateWorkspacePipeline, WorkspacePipeline};
use crate::query::search::ilike_contains;
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, PipelineStatus, WithAccountRef, keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating a workspace's pipelines: newest first by `created_at`,
/// `id` as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCursor {
    /// When the pipeline was created.
    pub created_at: Timestamp,
    /// Pipeline id (tiebreaker).
    pub id: uuid::Uuid,
}

/// Repository for pipeline database operations.
///
/// Handles pipeline lifecycle management including creation, updates,
/// status transitions, and queries.
pub trait WorkspacePipelineRepository {
    /// Creates a new pipeline record.
    fn create_workspace_pipeline(
        &mut self,
        new_pipeline: NewWorkspacePipeline,
    ) -> impl Future<Output = Result<WorkspacePipeline>> + Send;

    /// Finds a pipeline by slug within a specific workspace, with the handle and
    /// avatar of the account that created it.
    ///
    /// Excludes soft-deleted pipelines.
    fn find_pipeline_in_workspace_by_slug(
        &mut self,
        workspace_id: Uuid,
        slug: &str,
    ) -> impl Future<Output = Result<Option<WithAccountRef<WorkspacePipeline>>>> + Send;

    /// Lists all pipelines in a workspace with cursor pagination, each paired
    /// with the handle and avatar of the account that created it.
    fn cursor_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<PipelineCursor>,
        status_filter: Option<PipelineStatus>,
        search_term: Option<&str>,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspacePipeline>>>> + Send;

    /// Updates a pipeline with new data.
    fn update_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
        updates: UpdateWorkspacePipeline,
    ) -> impl Future<Output = Result<WorkspacePipeline>> + Send;

    /// Soft deletes a pipeline by setting the deletion timestamp.
    fn delete_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspacePipelineRepository for PgConnection {
    async fn create_workspace_pipeline(
        &mut self,
        new_pipeline: NewWorkspacePipeline,
    ) -> Result<WorkspacePipeline> {
        use schema::workspace_pipelines;

        let pipeline = diesel::insert_into(workspace_pipelines::table)
            .values(&new_pipeline)
            .returning(WorkspacePipeline::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(pipeline)
    }

    async fn find_pipeline_in_workspace_by_slug(
        &mut self,
        workspace_id: Uuid,
        slug: &str,
    ) -> Result<Option<WithAccountRef<WorkspacePipeline>>> {
        use schema::workspace_pipelines::dsl;
        use schema::{accounts, workspace_pipelines};

        let row = workspace_pipelines::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::slug.eq(slug))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspacePipeline::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .first::<(WorkspacePipeline, AccountRefRow)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|(item, account)| WithAccountRef { item, account }))
    }

    async fn cursor_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<PipelineCursor>,
        status_filter: Option<PipelineStatus>,
        search_term: Option<&str>,
    ) -> Result<CursorPage<WithAccountRef<WorkspacePipeline>>> {
        use schema::workspace_pipelines::dsl;
        use schema::{accounts, workspace_pipelines};

        // Build base query with filters
        let mut base_query = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply status filter
        if let Some(status) = status_filter {
            base_query = base_query.filter(dsl::status.eq(status));
        }

        // Hybrid name search: ILIKE substring (works for short queries) OR
        // trigram similarity (typo tolerance); both served by the trgm index.
        if let Some(term) = search_term {
            base_query = base_query.filter(
                dsl::display_name
                    .ilike(ilike_contains(term))
                    .or(dsl::display_name.trgm_similar_to(term)),
            );
        }

        let total = if pagination.include_count {
            Some(
                base_query
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        // Rebuild query for fetching items
        let mut query = workspace_pipelines::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        // Hybrid name search: ILIKE substring OR trigram similarity (see above).
        if let Some(term) = search_term {
            query = query.filter(
                dsl::display_name
                    .ilike(ilike_contains(term))
                    .or(dsl::display_name.trgm_similar_to(term)),
            );
        }

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let rows: Vec<(WorkspacePipeline, AccountRefRow)> =
            keyset!(query, dsl::created_at, dsl::id, pagination.direction, after)
                .select((
                    WorkspacePipeline::as_select(),
                    (
                        accounts::username,
                        accounts::display_name,
                        accounts::avatar_url,
                    ),
                ))
                .limit(pagination.fetch_limit())
                .load(self)
                .await
                .map_err(Error::from)?;

        let items: Vec<WithAccountRef<WorkspacePipeline>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            PipelineCursor {
                created_at: wc.item.created_at.into(),
                id: wc.item.id,
            }
        }))
    }

    async fn update_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
        updates: UpdateWorkspacePipeline,
    ) -> Result<WorkspacePipeline> {
        use schema::workspace_pipelines::{self, dsl};

        let pipeline = diesel::update(workspace_pipelines::table.filter(dsl::id.eq(pipeline_id)))
            .set(&updates)
            .returning(WorkspacePipeline::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(pipeline)
    }

    async fn delete_workspace_pipeline(&mut self, pipeline_id: Uuid) -> Result<()> {
        use diesel::dsl::now;
        use schema::workspace_pipelines::{self, dsl};

        diesel::update(workspace_pipelines::table.filter(dsl::id.eq(pipeline_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::model::{NewWorkspacePipeline, UpdateWorkspacePipeline};
    use crate::query::WorkspacePipelineRepository;
    use crate::test_util::TestDatabase;

    #[tokio::test]
    async fn create_find_update_and_soft_delete() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let pipeline = conn
            .create_workspace_pipeline(NewWorkspacePipeline::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let slug = pipeline.slug.as_str().to_owned();

        // Found by slug within its workspace, with the creator handle.
        let found = conn
            .find_pipeline_in_workspace_by_slug(seeded.workspace_id, &slug)
            .await?;
        assert_eq!(found.map(|p| p.item.id), Some(pipeline.id));

        // Not found in another workspace.
        assert!(
            conn.find_pipeline_in_workspace_by_slug(Uuid::now_v7(), &slug)
                .await?
                .is_none()
        );

        // Update the status.
        let updated = conn
            .update_workspace_pipeline(
                pipeline.id,
                UpdateWorkspacePipeline {
                    status: Some(PipelineStatus::Enabled),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(updated.status, PipelineStatus::Enabled);

        // Soft delete hides it from the by-slug lookup.
        conn.delete_workspace_pipeline(pipeline.id).await?;
        assert!(
            conn.find_pipeline_in_workspace_by_slug(seeded.workspace_id, &slug)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_filters_by_status_and_excludes_deleted() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A draft, an enabled, and a deleted pipeline.
        let draft = conn
            .create_workspace_pipeline(NewWorkspacePipeline::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let mut enabled = NewWorkspacePipeline::test(seeded.workspace_id, seeded.account_id);
        enabled.status = Some(PipelineStatus::Enabled);
        let enabled = conn.create_workspace_pipeline(enabled).await?;
        let deleted = conn
            .create_workspace_pipeline(NewWorkspacePipeline::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        conn.delete_workspace_pipeline(deleted.id).await?;

        // No filter: both live pipelines, deleted excluded.
        let all = conn
            .cursor_list_workspace_pipelines(
                seeded.workspace_id,
                CursorPagination::new(50),
                None,
                None,
            )
            .await?;
        let ids: Vec<_> = all.items.iter().map(|p| p.item.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&draft.id) && ids.contains(&enabled.id));

        // Filtered to Enabled.
        let enabled_only = conn
            .cursor_list_workspace_pipelines(
                seeded.workspace_id,
                CursorPagination::new(50),
                Some(PipelineStatus::Enabled),
                None,
            )
            .await?;
        assert_eq!(
            enabled_only
                .items
                .iter()
                .map(|p| p.item.id)
                .collect::<Vec<_>>(),
            vec![enabled.id]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_search_matches_display_name() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let mut invoices = NewWorkspacePipeline::test(seeded.workspace_id, seeded.account_id);
        invoices.display_name = "Invoice Redaction".to_owned();
        let invoices = conn.create_workspace_pipeline(invoices).await?;
        let mut contracts = NewWorkspacePipeline::test(seeded.workspace_id, seeded.account_id);
        contracts.display_name = "Contract Review".to_owned();
        let _ = conn.create_workspace_pipeline(contracts).await?;

        // A substring search finds only the matching pipeline.
        let page = conn
            .cursor_list_workspace_pipelines(
                seeded.workspace_id,
                CursorPagination::new(50),
                None,
                Some("invoice"),
            )
            .await?;
        assert_eq!(
            page.items.iter().map(|p| p.item.id).collect::<Vec<_>>(),
            vec![invoices.id]
        );
        Ok(())
    }
}
