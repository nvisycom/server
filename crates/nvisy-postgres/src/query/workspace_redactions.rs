//! Workspace redactions repository for managing redaction instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{NewWorkspaceRedaction, WorkspaceRedaction};
use crate::types::{CursorPage, CursorPagination, RedactionFilter, keyset};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating a detection's redactions: newest first by `created_at`,
/// `id` as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionCursor {
    /// When the redaction was created.
    pub created_at: Timestamp,
    /// Redaction id (tiebreaker).
    pub id: uuid::Uuid,
}

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
    /// A [`RedactionId`] is globally unique, so a redaction is addressable by id
    /// alone; this resolves it only within the given workspace by joining through
    /// its detection's own workspace, so an ad-hoc detection (no pipeline) or one
    /// whose pipeline was deleted still resolves.
    ///
    /// [`RedactionId`]: crate::types::RedactionId
    fn find_redaction_in_workspace(
        &mut self,
        workspace_id: Uuid,
        redaction_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceRedaction>>> + Send;

    /// Lists a detection's redactions with cursor pagination, newest first.
    fn cursor_list_detection_redactions(
        &mut self,
        detection_id: Uuid,
        pagination: CursorPagination<RedactionCursor>,
    ) -> impl Future<Output = Result<CursorPage<WorkspaceRedaction>>> + Send;

    /// Lists a workspace's redactions with cursor pagination, newest first,
    /// narrowed by an optional detection and document.
    ///
    /// Redactions carry no workspace column; the scope (and the `document_id`
    /// filter) is applied through the redaction's detection, so an ad-hoc
    /// detection (no pipeline) or one whose pipeline was deleted still resolves.
    fn cursor_list_workspace_redactions(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<RedactionCursor>,
        filter: &RedactionFilter,
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
        use schema::workspace_detections::dsl as detections;
        use schema::workspace_redactions::dsl as redactions;
        use schema::{workspace_detections, workspace_redactions};

        // Redactions carry no workspace column; scope through the detection's own
        // workspace so the id resolves only within its workspace, including an
        // ad-hoc detection (no pipeline) and one whose pipeline was deleted.
        let redaction = workspace_redactions::table
            .inner_join(workspace_detections::table)
            .filter(redactions::id.eq(redaction_id))
            .filter(redactions::deleted_at.is_null())
            .filter(detections::workspace_id.eq(workspace_id))
            .filter(detections::deleted_at.is_null())
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
        pagination: CursorPagination<RedactionCursor>,
    ) -> Result<CursorPage<WorkspaceRedaction>> {
        use schema::workspace_redactions::{self, dsl};

        let base_query = workspace_redactions::table
            .filter(dsl::detection_id.eq(detection_id))
            .filter(dsl::deleted_at.is_null());

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

        let scoped = workspace_redactions::table
            .filter(dsl::detection_id.eq(detection_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let items: Vec<WorkspaceRedaction> = keyset!(
            scoped,
            dsl::created_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select(WorkspaceRedaction::as_select())
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            RedactionCursor {
                created_at: row.created_at.into(),
                id: row.id,
            }
        }))
    }

    async fn cursor_list_workspace_redactions(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<RedactionCursor>,
        filter: &RedactionFilter,
    ) -> Result<CursorPage<WorkspaceRedaction>> {
        use schema::workspace_detections::dsl as detections;
        use schema::workspace_redactions::dsl as redactions;
        use schema::{workspace_detections, workspace_redactions};

        // One scoped builder for both the count and the page, so a future filter
        // cannot be added to one and forgotten on the other. Redactions carry no
        // workspace column, so scope (and the document filter) run through the
        // detection: `redactions ⋈ detections` on `workspace_id`, and on the
        // detection's `input_document_id` when a document is given.
        let scoped = || {
            let mut query = workspace_redactions::table
                .inner_join(workspace_detections::table)
                .filter(detections::workspace_id.eq(workspace_id))
                .filter(detections::deleted_at.is_null())
                .filter(redactions::deleted_at.is_null())
                .into_boxed();
            if let Some(detection_id) = filter.detection_id {
                query = query.filter(redactions::detection_id.eq(detection_id));
            }
            if let Some(document_id) = filter.document_id {
                query = query.filter(detections::input_document_id.eq(document_id));
            }
            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let items: Vec<WorkspaceRedaction> = keyset!(
            scoped(),
            redactions::created_at,
            redactions::id,
            pagination.direction,
            after
        )
        .select(WorkspaceRedaction::as_select())
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            RedactionCursor {
                created_at: row.created_at.into(),
                id: row.id,
            }
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
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;
        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.pipeline_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        Ok((
            seeded.account_id,
            seeded.workspace_id,
            seeded.pipeline_id,
            detection.id,
        ))
    }

    #[tokio::test]
    async fn create_then_find_is_scoped_to_the_detection_workspace() -> anyhow::Result<()> {
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
    async fn a_redaction_resolves_after_its_pipeline_is_soft_deleted() -> anyhow::Result<()> {
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

        // The redaction is scoped by its detection's workspace, not its pipeline,
        // so soft-deleting the owning pipeline leaves it discoverable.
        conn.delete_workspace_pipeline(pipeline_id).await?;
        assert!(
            conn.find_redaction_in_workspace(workspace_id, redaction.id)
                .await?
                .is_some()
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

    #[tokio::test]
    async fn cursor_list_workspace_redactions_scopes_and_filters() -> anyhow::Result<()> {
        use crate::model::{NewBlob, NewWorkspaceDocument};
        use crate::query::WorkspaceDocumentRepository;
        use crate::types::RedactionFilter;

        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        // Two detections in the same workspace over two different documents, each
        // with its own redaction. The second document is created here; the first is
        // the seeded one.
        let doc_a = seeded.document_id;
        let doc_b = conn
            .create_workspace_document(
                NewWorkspaceDocument::test(seeded.workspace_id, seeded.account_id, Uuid::nil()),
                NewBlob::test(seeded.workspace_id),
            )
            .await?
            .id;
        let detection_a = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.pipeline_id,
                seeded.account_id,
                doc_a,
            ))
            .await?
            .id;
        let detection_b = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.pipeline_id,
                seeded.account_id,
                doc_b,
            ))
            .await?
            .id;
        let redaction_a = conn
            .create_redaction(NewWorkspaceRedaction::test(detection_a, seeded.account_id))
            .await?;
        let redaction_b = conn
            .create_redaction(NewWorkspaceRedaction::test(detection_b, seeded.account_id))
            .await?;

        // Unfiltered: the workspace's listing returns both.
        let all = conn
            .cursor_list_workspace_redactions(
                seeded.workspace_id,
                CursorPagination::new(50),
                &RedactionFilter::default(),
            )
            .await?;
        let mut ids = all.items.iter().map(|r| r.id).collect::<Vec<_>>();
        ids.sort();
        let mut want = vec![redaction_a.id, redaction_b.id];
        want.sort();
        assert_eq!(ids, want, "both redactions are listed for the workspace");

        // Filter by document: only that document's detection's redaction.
        let by_doc = conn
            .cursor_list_workspace_redactions(
                seeded.workspace_id,
                CursorPagination::new(50),
                &RedactionFilter {
                    document_id: Some(doc_a),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            by_doc.items.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![redaction_a.id],
            "the document filter scopes through the detection"
        );

        // Filter by detection: only that detection's redaction.
        let by_detection = conn
            .cursor_list_workspace_redactions(
                seeded.workspace_id,
                CursorPagination::new(50),
                &RedactionFilter {
                    detection_id: Some(detection_b),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            by_detection.items.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![redaction_b.id]
        );

        // A different workspace sees none of them.
        let other = conn
            .cursor_list_workspace_redactions(
                Uuid::now_v7(),
                CursorPagination::new(50),
                &RedactionFilter::default(),
            )
            .await?;
        assert!(other.items.is_empty());
        Ok(())
    }
}
