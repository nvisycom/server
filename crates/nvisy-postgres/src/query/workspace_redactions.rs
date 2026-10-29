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

/// Redacted output metadata for one detection, used by the artifacts endpoint.
#[derive(Debug, Clone)]
pub struct DetectionRedactedOutput {
    /// Redaction pass that produced the output.
    pub redaction_id: Uuid,
    /// Output document id.
    pub document_id: Uuid,
    /// Display name of the output document.
    pub display_name: String,
    /// File extension without the dot.
    pub extension: String,
    /// Plaintext size in bytes.
    pub size: i64,
}

/// Repository for workspace redaction database operations.
///
/// A redaction is one redact pass over a detection's analysis; a detection can
/// have many. Each redaction owns the review audit it applied and the redacted
/// document it produced.
pub trait WorkspaceRedactionRepository {
    /// Lists live redacted output files for a detection, newest first.
    fn detection_outputs(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> impl Future<Output = Result<Vec<DetectionRedactedOutput>>> + Send;

    /// Creates a new workspace redaction record.
    fn create_redaction(
        &mut self,
        new_redaction: NewWorkspaceRedaction,
    ) -> impl Future<Output = Result<WorkspaceRedaction>> + Send;

    /// Finds a redaction by its id, scoped to a workspace.
    ///
    /// A [`RedactionId`] is globally unique, so a redaction is addressable by id
    /// alone; this resolves it only within the given workspace by joining through
    /// its detection's own workspace.
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
    /// filter) is applied through the redaction's detection within its workspace.
    fn cursor_list_workspace_redactions(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<RedactionCursor>,
        filter: &RedactionFilter,
    ) -> impl Future<Output = Result<CursorPage<WorkspaceRedaction>>> + Send;
}

impl WorkspaceRedactionRepository for PgConnection {
    async fn detection_outputs(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> Result<Vec<DetectionRedactedOutput>> {
        use schema::workspace_detections::dsl as detections;
        use schema::workspace_documents::dsl as documents;
        use schema::workspace_redactions::dsl as redactions;
        use schema::{
            workspace_blobs, workspace_detections, workspace_documents, workspace_redactions,
        };

        let rows = workspace_redactions::table
            .inner_join(workspace_detections::table)
            .inner_join(
                workspace_documents::table
                    .on(redactions::output_document_id.eq(documents::id.nullable())),
            )
            .inner_join(
                workspace_blobs::table.on(documents::blob_id.eq(workspace_blobs::id.nullable())),
            )
            .filter(detections::workspace_id.eq(workspace_id))
            .filter(detections::deleted_at.is_null())
            .filter(redactions::deleted_at.is_null())
            .filter(documents::deleted_at.is_null())
            .filter(documents::workspace_id.eq(workspace_id))
            .filter(workspace_blobs::workspace_id.eq(workspace_id))
            .filter(workspace_blobs::purged_at.is_null())
            .order((redactions::created_at.desc(), redactions::id.desc()))
            .filter(redactions::detection_id.eq(detection_id))
            .select((
                redactions::id,
                documents::id,
                documents::display_name,
                documents::file_extension,
                workspace_blobs::file_size_bytes,
            ))
            .load::<(Uuid, Uuid, String, String, i64)>(self)
            .await
            .map_err(Error::from)?;
        Ok(rows
            .into_iter()
            .map(
                |(redaction_id, document_id, display_name, extension, size)| {
                    DetectionRedactedOutput {
                        redaction_id,
                        document_id,
                        display_name,
                        extension,
                        size,
                    }
                },
            )
            .collect())
    }

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
        // workspace so the id resolves only within its workspace.
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
    use crate::query::{WorkspaceDetectionRepository, WorkspaceRedactionRepository};
    use crate::test_util::{TestDatabase, backdate};

    #[tokio::test]
    async fn detection_outputs_exclude_other_workspaces_and_deleted_files() -> anyhow::Result<()> {
        use crate::model::{NewBlob, NewWorkspaceDocument};
        use crate::query::WorkspaceDocumentRepository;
        use crate::types::DocumentKind;

        let db = TestDatabase::start().await;
        let (account_id, workspace_id, detection_id) = seed_detection(&db).await?;
        let mut conn = db.client.get_connection().await?;
        let mut document = NewWorkspaceDocument::test(workspace_id, account_id, Uuid::nil());
        document.kind = Some(DocumentKind::Redacted);
        document.display_name = Some("redacted.txt".into());
        document.file_extension = Some("txt".into());
        let document = conn
            .create_workspace_document(document, NewBlob::test(workspace_id))
            .await?;
        let mut redaction = NewWorkspaceRedaction::test(detection_id, account_id);
        redaction.output_document_id = Some(document.id);
        let redaction = conn.create_redaction(redaction).await?;

        let outputs = conn.detection_outputs(workspace_id, detection_id).await?;
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].document_id, document.id);
        assert_eq!(outputs[0].redaction_id, redaction.id);
        assert_eq!(outputs[0].display_name, "redacted.txt");
        assert_eq!(outputs[0].extension, "txt");
        assert!(
            conn.detection_outputs(Uuid::new_v4(), detection_id)
                .await?
                .is_empty()
        );
        assert!(
            conn.detection_outputs(workspace_id, Uuid::new_v4())
                .await?
                .is_empty()
        );

        conn.delete_workspace_document(document.id).await?;
        assert!(
            conn.detection_outputs(workspace_id, detection_id)
                .await?
                .is_empty()
        );
        Ok(())
    }

    /// Seeds a detection and returns `(account_id, workspace_id, detection_id)` — a
    /// redaction's FK parent plus the context tests scope on.
    async fn seed_detection(db: &TestDatabase) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;
        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        Ok((seeded.account_id, seeded.workspace_id, detection.id))
    }

    #[tokio::test]
    async fn create_then_find_is_scoped_to_the_detection_workspace() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, workspace_id, detection_id) = seed_detection(&db).await?;
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
    async fn cursor_list_returns_a_detections_redactions_newest_first() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, _ws, detection_id) = seed_detection(&db).await?;
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
        assert_eq!(empty.items, vec![]);
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_workspace_redactions_scopes_and_filters() -> anyhow::Result<()> {
        use crate::model::{NewBlob, NewWorkspaceDocument};
        use crate::query::WorkspaceDocumentRepository;
        use crate::types::RedactionFilter;

        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
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
                seeded.account_id,
                doc_a,
            ))
            .await?
            .id;
        let detection_b = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
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
        assert_eq!(other.items, vec![]);
        Ok(())
    }
}
