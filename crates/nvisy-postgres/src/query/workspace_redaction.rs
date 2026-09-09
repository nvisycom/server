//! Workspace redactions repository for managing redaction instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceRedaction, WorkspaceRedaction};
use crate::types::{CursorPage, CursorPagination};
use crate::{Error, PgConnection, Result, schema};

/// Repository for workspace redaction database operations.
///
/// A redaction is one redact pass over a detection's analysis; a detection can
/// have many. Each redaction owns the review audit it applied and the redacted
/// document it produced.
pub trait WorkspaceRedactionRepository {
    /// Creates a new workspace redaction record.
    fn create_redaction(
        &mut self,
        new_redaction: NewWorkspaceRedaction,
    ) -> impl Future<Output = Result<WorkspaceRedaction>> + Send;

    /// Finds a redaction by its id, scoped to a workspace.
    ///
    /// A [`RedactionId`](crate::types::RedactionId) is globally unique, so a
    /// redaction is addressable by id alone; this resolves it only within the
    /// given workspace by joining through its detection's pipeline.
    fn find_redaction_in_workspace(
        &mut self,
        workspace_id: Uuid,
        redaction_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceRedaction>>> + Send;

    /// Lists a detection's redactions with cursor pagination, newest first.
    fn cursor_list_detection_redactions(
        &mut self,
        detection_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = Result<CursorPage<WorkspaceRedaction>>> + Send;
}

impl WorkspaceRedactionRepository for PgConnection {
    async fn create_redaction(
        &mut self,
        new_redaction: NewWorkspaceRedaction,
    ) -> Result<WorkspaceRedaction> {
        use schema::workspace_redactions;

        let redaction = diesel::insert_into(workspace_redactions::table)
            .values(&new_redaction)
            .returning(WorkspaceRedaction::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(redaction)
    }

    async fn find_redaction_in_workspace(
        &mut self,
        workspace_id: Uuid,
        redaction_id: Uuid,
    ) -> Result<Option<WorkspaceRedaction>> {
        use schema::workspace_redactions::dsl as redactions;
        use schema::{workspace_detections, workspace_pipelines, workspace_redactions};

        // Redactions carry no workspace column; scope through the detection's
        // pipeline so the id resolves only within its workspace, and only while
        // that pipeline is live (a soft-deleted pipeline hides its redactions).
        let redaction = workspace_redactions::table
            .inner_join(workspace_detections::table.inner_join(workspace_pipelines::table))
            .filter(redactions::id.eq(redaction_id))
            .filter(workspace_pipelines::workspace_id.eq(workspace_id))
            .filter(workspace_pipelines::deleted_at.is_null())
            .select(WorkspaceRedaction::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(redaction)
    }

    async fn cursor_list_detection_redactions(
        &mut self,
        detection_id: Uuid,
        pagination: CursorPagination,
    ) -> Result<CursorPage<WorkspaceRedaction>> {
        use schema::workspace_redactions::{self, dsl};

        let base_query = workspace_redactions::table.filter(dsl::detection_id.eq(detection_id));

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

        let limit = pagination.fetch_limit();

        let items: Vec<WorkspaceRedaction> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            workspace_redactions::table
                .filter(dsl::detection_id.eq(detection_id))
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(WorkspaceRedaction::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(Error::from)?
        } else {
            workspace_redactions::table
                .filter(dsl::detection_id.eq(detection_id))
                .select(WorkspaceRedaction::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(Error::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            (row.created_at.into(), row.id)
        }))
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};
    use uuid::Uuid;

    use super::*;
    use crate::model::{NewWorkspaceDetection, NewWorkspaceRedaction};
    use crate::query::{
        WorkspaceDetectionRepository, WorkspacePipelineRepository, WorkspaceRedactionRepository,
    };
    use crate::test_util::{TestDatabase, backdate};

    /// Seeds a detection and returns `(account_id, workspace_id, pipeline_id,
    /// detection_id)` — a redaction's FK parent plus the context tests scope on.
    async fn seed_detection(db: &TestDatabase) -> anyhow::Result<(Uuid, Uuid, Uuid, Uuid)> {
        let (account_id, workspace_id, pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;
        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                pipeline_id,
                account_id,
                file_id,
            ))
            .await?;
        Ok((account_id, workspace_id, pipeline_id, detection.id))
    }

    #[tokio::test]
    async fn create_then_find_is_scoped_through_the_pipeline() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, workspace_id, _pipeline, detection_id) = seed_detection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        let redaction = conn
            .create_redaction(NewWorkspaceRedaction::test(detection_id, account_id))
            .await?;

        // Found within its own workspace.
        assert!(
            conn.find_redaction_in_workspace(workspace_id, redaction.id)
                .await?
                .is_some()
        );
        // Not found scoped to a different workspace.
        assert!(
            conn.find_redaction_in_workspace(Uuid::now_v7(), redaction.id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_soft_deleted_pipeline_hides_its_redactions() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, workspace_id, pipeline_id, detection_id) = seed_detection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        let redaction = conn
            .create_redaction(NewWorkspaceRedaction::test(detection_id, account_id))
            .await?;
        assert!(
            conn.find_redaction_in_workspace(workspace_id, redaction.id)
                .await?
                .is_some()
        );

        // Soft-deleting the owning pipeline hides the redaction from the lookup.
        conn.delete_workspace_pipeline(pipeline_id).await?;
        assert!(
            conn.find_redaction_in_workspace(workspace_id, redaction.id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_returns_a_detections_redactions_newest_first() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, _ws, _pipeline, detection_id) = seed_detection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        // Backdate `first` an hour so the newest-first order is deterministic.
        let first = conn
            .create_redaction(NewWorkspaceRedaction::test(detection_id, account_id))
            .await?;
        backdate::redaction_created_at(
            &mut conn,
            first.id,
            Timestamp::now() - Span::new().hours(1),
        )
        .await?;
        let second = conn
            .create_redaction(NewWorkspaceRedaction::test(detection_id, account_id))
            .await?;

        let page = conn
            .cursor_list_detection_redactions(detection_id, CursorPagination::new(50))
            .await?;
        // Both belong to the detection, newest first.
        assert_eq!(
            page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![second.id, first.id]
        );

        // A different detection has none of them.
        let empty = conn
            .cursor_list_detection_redactions(Uuid::now_v7(), CursorPagination::new(50))
            .await?;
        assert!(empty.items.is_empty());
        Ok(())
    }
}
