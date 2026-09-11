//! Workspace files repository for managing uploaded files.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    NewWorkspaceFile, NewWorkspaceFileExport, NewWorkspaceFileImport, UpdateWorkspaceFile,
    WorkspaceFile,
};
use crate::query::search::ilike_contains;
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, DetectionStatus, FileFilter, FileKind,
    WithAccountRef, keyset,
};

/// Keyset for paginating a workspace's files: newest first by `created_at`, `id`
/// as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCursor {
    /// When the file was created.
    pub created_at: Timestamp,
    /// File id (tiebreaker).
    pub id: uuid::Uuid,
}
use crate::{Error, PgConnection, Result, schema};

/// A live file imported from a connection, for deletion reconciliation.
///
/// A `source_key` no longer present in the remote listing identifies a file
/// whose source object was removed.
#[derive(Debug, Clone, Queryable)]
pub struct ImportedFileRef {
    /// The connection-side key the file was imported from.
    pub source_key: String,
    /// The imported file's id.
    pub file_id: Uuid,
    /// The imported file's object-store path.
    pub storage_path: String,
}

/// A file whose retention window has elapsed, for the expiry sweep.
#[derive(Debug, Clone, Queryable)]
pub struct ExpiredFileRef {
    /// The file's id.
    pub id: Uuid,
    /// The file's object-store path.
    pub storage_path: String,
    /// The bucket the file's object lives in.
    pub storage_bucket: String,
}

/// Repository for workspace file database operations.
///
/// Handles file lifecycle management including upload tracking,
/// storage management, and cleanup operations.
pub trait WorkspaceFileRepository {
    /// Creates a new workspace file record.
    fn create_workspace_file(
        &mut self,
        new_file: NewWorkspaceFile,
    ) -> impl Future<Output = Result<WorkspaceFile>> + Send;

    /// Creates a workspace file and records its import origin (the connection and
    /// remote object key it came from) in a single transaction.
    fn record_imported_file(
        &mut self,
        new_file: NewWorkspaceFile,
        connection_id: Uuid,
        source_key: String,
    ) -> impl Future<Output = Result<WorkspaceFile>> + Send;

    /// Finds a workspace file by its unique identifier.
    fn find_workspace_file_by_id(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceFile>>> + Send;

    /// Finds a file by ID within a specific workspace.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_file_in_workspace(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceFile>>> + Send;

    /// Finds a file by id within a workspace, with the handle and avatar of the
    /// account that uploaded it.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_file_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = Result<Option<WithAccountRef<WorkspaceFile>>>> + Send;

    /// Returns the remote object keys already imported (live) from a connection.
    ///
    /// Used to skip re-importing objects during a connection sync.
    fn imported_keys_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Vec<String>>> + Send;

    /// Records that `file_id` was exported to `connection_id` under `remote_key`,
    /// so a scheduled redacted export does not push it again.
    fn record_exported_file(
        &mut self,
        file_id: Uuid,
        connection_id: Uuid,
        remote_key: String,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Returns the live redacted files in `connection`'s workspace that have not
    /// yet been exported to that connection. Backs scheduled export.
    fn redacted_files_not_exported(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WorkspaceFile>>> + Send;

    /// Returns each live file imported from a connection, for deletion
    /// reconciliation (a source key absent from the remote listing identifies a
    /// removed source object).
    fn imported_files_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Vec<ImportedFileRef>>> + Send;

    /// Returns up to `limit` live files whose retention window has elapsed
    /// (`expires_at < now`). The file reaper sweeps these, purges their objects,
    /// and soft-deletes the rows.
    fn files_due_for_expiry(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<ExpiredFileRef>>> + Send;

    /// Returns up to `limit` soft-deleted files whose backing object has not yet
    /// been reclaimed (`deleted_at IS NOT NULL AND purged_at IS NULL`). The reaper
    /// sweeps these to purge objects that a best-effort delete missed, or that a
    /// delete path left behind — retried until `purged_at` is stamped.
    fn files_pending_purge(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<ExpiredFileRef>>> + Send;

    /// Stamps `purged_at = now()` once a file's backing object is removed, taking
    /// the row out of the reaper's pending-purge set. Idempotent: only sets it
    /// when currently NULL.
    fn mark_file_purged(&mut self, file_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Updates a workspace file with new metadata or settings.
    fn update_workspace_file(
        &mut self,
        file_id: Uuid,
        updates: UpdateWorkspaceFile,
    ) -> impl Future<Output = Result<WorkspaceFile>> + Send;

    /// Soft deletes a workspace file by setting the deletion timestamp.
    fn delete_workspace_file(&mut self, file_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Lists all files in a workspace with cursor pagination and optional
    /// filtering, each paired with the handle and avatar of the account that
    /// uploaded it.
    fn cursor_list_workspace_files(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<FileCursor>,
        filter: FileFilter,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceFile>>>> + Send;

    /// Soft-deletes the live files among `file_ids` that belong to
    /// `workspace_id`, returning the rows it actually transitioned.
    ///
    /// Resolution and deletion are one atomic step: the `UPDATE ... RETURNING`
    /// guarded on `deleted_at IS NULL` transitions and returns only rows it
    /// changed, so a row concurrently deleted by another request is absent from
    /// the result and never double-reported. Ids that are unknown, already
    /// deleted, or in another workspace are simply absent rather than an error.
    /// Also drops each returned file's import-origin row so re-import is never
    /// blocked (see [`delete_workspace_file`]).
    ///
    /// [`delete_workspace_file`]: WorkspaceFileRepository::delete_workspace_file
    fn delete_files_in_workspace(
        &mut self,
        workspace_id: Uuid,
        file_ids: &[Uuid],
    ) -> impl Future<Output = Result<Vec<WorkspaceFile>>> + Send;
}

impl WorkspaceFileRepository for PgConnection {
    async fn create_workspace_file(&mut self, new_file: NewWorkspaceFile) -> Result<WorkspaceFile> {
        use schema::workspace_files;

        let file = diesel::insert_into(workspace_files::table)
            .values(&new_file)
            .returning(WorkspaceFile::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(file)
    }

    async fn record_imported_file(
        &mut self,
        new_file: NewWorkspaceFile,
        connection_id: Uuid,
        source_key: String,
    ) -> Result<WorkspaceFile> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_file_imports, workspace_files};

        self.transaction(async |conn| {
            let file = diesel::insert_into(workspace_files::table)
                .values(&new_file)
                .returning(WorkspaceFile::as_returning())
                .get_result(conn)
                .await
                .map_err(Error::from)?;

            diesel::insert_into(workspace_file_imports::table)
                .values(NewWorkspaceFileImport {
                    file_id: file.id,
                    connection_id,
                    source_key,
                })
                .execute(conn)
                .await
                .map_err(Error::from)?;

            Ok::<_, Error>(file)
        })
        .await
    }

    async fn find_workspace_file_by_id(&mut self, file_id: Uuid) -> Result<Option<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        let file = workspace_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceFile::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(file)
    }

    async fn find_file_in_workspace(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        let file = workspace_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceFile::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(file)
    }

    async fn imported_keys_for_connection(&mut self, connection_id: Uuid) -> Result<Vec<String>> {
        use schema::{workspace_file_imports, workspace_files};

        let keys = workspace_file_imports::table
            .inner_join(workspace_files::table)
            .filter(workspace_file_imports::connection_id.eq(connection_id))
            .filter(workspace_files::deleted_at.is_null())
            .select(workspace_file_imports::source_key)
            .load::<String>(self)
            .await
            .map_err(Error::from)?;

        Ok(keys)
    }

    async fn record_exported_file(
        &mut self,
        file_id: Uuid,
        connection_id: Uuid,
        remote_key: String,
    ) -> Result<()> {
        use schema::workspace_file_exports;

        diesel::insert_into(workspace_file_exports::table)
            .values(NewWorkspaceFileExport {
                file_id,
                connection_id,
                remote_key,
            })
            .on_conflict((
                workspace_file_exports::file_id,
                workspace_file_exports::connection_id,
            ))
            .do_update()
            .set((
                workspace_file_exports::remote_key
                    .eq(diesel::upsert::excluded(workspace_file_exports::remote_key)),
                workspace_file_exports::exported_at.eq(diesel::dsl::now),
            ))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn redacted_files_not_exported(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<Vec<WorkspaceFile>> {
        use schema::workspace_file_exports;
        use schema::workspace_files::{self, dsl};

        let exported = workspace_file_exports::table
            .filter(workspace_file_exports::connection_id.eq(connection_id))
            .select(workspace_file_exports::file_id);

        let files = workspace_files::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::file_kind.eq(FileKind::Redacted))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::id.ne_all(exported))
            .select(WorkspaceFile::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(files)
    }

    async fn imported_files_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> Result<Vec<ImportedFileRef>> {
        use schema::{workspace_file_imports, workspace_files};

        let files = workspace_file_imports::table
            .inner_join(workspace_files::table)
            .filter(workspace_file_imports::connection_id.eq(connection_id))
            .filter(workspace_files::deleted_at.is_null())
            .select((
                workspace_file_imports::source_key,
                workspace_files::id,
                workspace_files::storage_path,
            ))
            .load::<ImportedFileRef>(self)
            .await
            .map_err(Error::from)?;

        Ok(files)
    }

    async fn files_due_for_expiry(&mut self, limit: i64) -> Result<Vec<ExpiredFileRef>> {
        use diesel::dsl::{exists, not, now};
        use schema::workspace_detections::dsl as detections;
        use schema::{workspace_detections, workspace_files};

        // A detection that is still analyzing (pending or executing) needs its
        // input document and audit blob, so those files are held back from expiry
        // until analysis reaches a terminal state — otherwise an in-flight detect
        // could lose its source or analysis mid-flight and get stuck. A `Complete`
        // detection is NOT held: it is terminal (redaction never changes its
        // status), so holding it would pin the input and audit forever and defeat
        // retention. Re-redaction of a complete detection is bounded by those
        // files' own `expires_at` — retention itself decides how long they remain
        // redactable. Redaction outputs are not protected here — they belong to
        // redactions, not the detection.
        let active_detection_holds_file = exists(
            workspace_detections::table.filter(
                detections::status.eq_any(DetectionStatus::IN_PROGRESS).and(
                    detections::input_file_id
                        .eq(workspace_files::id)
                        .or(detections::audit_file_id.eq(workspace_files::id.nullable())),
                ),
            ),
        );

        let files = workspace_files::table
            .filter(workspace_files::expires_at.is_not_null())
            .filter(workspace_files::expires_at.lt(now))
            .filter(workspace_files::deleted_at.is_null())
            .filter(not(active_detection_holds_file))
            .select((
                workspace_files::id,
                workspace_files::storage_path,
                workspace_files::storage_bucket,
            ))
            .limit(limit)
            .load::<ExpiredFileRef>(self)
            .await
            .map_err(Error::from)?;

        Ok(files)
    }

    async fn files_pending_purge(&mut self, limit: i64) -> Result<Vec<ExpiredFileRef>> {
        use schema::workspace_files;

        let files = workspace_files::table
            .filter(workspace_files::deleted_at.is_not_null())
            .filter(workspace_files::purged_at.is_null())
            .select((
                workspace_files::id,
                workspace_files::storage_path,
                workspace_files::storage_bucket,
            ))
            .limit(limit)
            .load::<ExpiredFileRef>(self)
            .await
            .map_err(Error::from)?;

        Ok(files)
    }

    async fn mark_file_purged(&mut self, file_id: Uuid) -> Result<()> {
        use schema::workspace_files::{self, dsl};

        diesel::update(workspace_files::table)
            .filter(dsl::id.eq(file_id))
            .filter(dsl::purged_at.is_null())
            .set(dsl::purged_at.eq(diesel::dsl::now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn find_file_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<WithAccountRef<WorkspaceFile>>> {
        use schema::workspace_files::dsl;
        use schema::{accounts, workspace_files};

        let row = workspace_files::table
            .inner_join(accounts::table)
            .filter(dsl::id.eq(file_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspaceFile::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .first::<(WorkspaceFile, AccountRefRow)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|(item, account)| WithAccountRef { item, account }))
    }

    async fn update_workspace_file(
        &mut self,
        file_id: Uuid,
        updates: UpdateWorkspaceFile,
    ) -> Result<WorkspaceFile> {
        use schema::workspace_files::{self, dsl};

        let file = diesel::update(workspace_files::table.filter(dsl::id.eq(file_id)))
            .set(&updates)
            .returning(WorkspaceFile::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(file)
    }

    async fn delete_workspace_file(&mut self, file_id: Uuid) -> Result<()> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_file_imports, workspace_files};

        // Soft-delete the file and drop its import-origin row (if any) atomically.
        // The origin's `(connection_id, source_key)` uniqueness would otherwise
        // block ever re-importing that source object, since the soft delete keeps
        // the file row and its `ON DELETE CASCADE` never fires.
        //
        // Guarded on `deleted_at IS NULL` so it is idempotent: the reaper's
        // reconcile sweep re-invokes this for an already-deleted row, and must not
        // move `deleted_at` forward.
        self.transaction(async |conn| {
            diesel::update(
                workspace_files::table
                    .filter(workspace_files::id.eq(file_id))
                    .filter(workspace_files::deleted_at.is_null()),
            )
            .set(workspace_files::deleted_at.eq(diesel::dsl::now))
            .execute(conn)
            .await
            .map_err(Error::from)?;

            diesel::delete(
                workspace_file_imports::table.filter(workspace_file_imports::file_id.eq(file_id)),
            )
            .execute(conn)
            .await
            .map_err(Error::from)?;

            Ok::<_, Error>(())
        })
        .await
    }

    async fn cursor_list_workspace_files(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<FileCursor>,
        filter: FileFilter,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceFile>>> {
        use schema::workspace_files::dsl;
        use schema::{accounts, workspace_files};

        // Precompute filter values
        let search_term = filter.search.clone();
        let extensions = filter.extensions.clone();
        let hash = filter.hash.clone();

        // The scoped builder (filters shared by the count and the page). The
        // document-kind filter is what keeps audit/artifact rows out of the list,
        // so it must apply to both or the count and items disagree.
        let scoped = || {
            let mut query = workspace_files::table
                .inner_join(accounts::table)
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::file_kind.eq_any(FileKind::DOCUMENTS))
                .into_boxed();

            // Hybrid name search: ILIKE substring (works for short queries) OR
            // trigram similarity (typo tolerance); both served by the trgm index.
            if let Some(ref term) = search_term {
                query = query.filter(
                    dsl::display_name
                        .ilike(ilike_contains(term))
                        .or(dsl::display_name.trgm_similar_to(term)),
                );
            }

            // Apply the extension constraint. A present-but-empty set matches
            // nothing (an active facet with no members), so apply whenever `Some`.
            if let Some(ref extensions) = extensions {
                query = query.filter(dsl::file_extension.eq_any(extensions));
            }

            // Apply the exact content-hash constraint (dedup lookup).
            if let Some(ref hash) = hash {
                query = query.filter(dsl::file_hash_sha256.eq(hash));
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
        let rows: Vec<(WorkspaceFile, AccountRefRow)> = keyset!(
            scoped(),
            dsl::created_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select((
            WorkspaceFile::as_select(),
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

        let items: Vec<WithAccountRef<WorkspaceFile>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            FileCursor {
                created_at: wc.item.created_at.into(),
                id: wc.item.id,
            }
        }))
    }

    async fn delete_files_in_workspace(
        &mut self,
        workspace_id: Uuid,
        file_ids: &[Uuid],
    ) -> Result<Vec<WorkspaceFile>> {
        use diesel::dsl::{exists, not};
        use schema::workspace_detections::dsl as detections;
        use schema::{workspace_detections, workspace_file_imports, workspace_files};

        // A detection still analyzing (pending or executing) needs its input
        // document and audit blob, so a file either references is held back from
        // deletion — otherwise purging it would strand the in-flight detection with
        // no source or analysis. This mirrors the expiry sweep's hold
        // (`files_due_for_expiry`); a held file is simply not transitioned, so it
        // is absent from the result and the caller reports it as skipped.
        let active_detection_holds_file = exists(
            workspace_detections::table.filter(
                detections::status.eq_any(DetectionStatus::IN_PROGRESS).and(
                    detections::input_file_id
                        .eq(workspace_files::id)
                        .or(detections::audit_file_id.eq(workspace_files::id.nullable())),
                ),
            ),
        );

        // Transition and return only the live, non-held rows in this workspace, in
        // one atomic statement. The `deleted_at IS NULL` guard means a row a
        // concurrent request already deleted is not returned here, so it is never
        // double-counted or double-emitted. This runs on the caller's connection
        // (not its own transaction) so the caller can commit the deletion together
        // with the events it emits for the returned rows.
        let deleted: Vec<WorkspaceFile> = diesel::update(
            workspace_files::table
                .filter(workspace_files::id.eq_any(file_ids))
                .filter(workspace_files::workspace_id.eq(workspace_id))
                .filter(workspace_files::deleted_at.is_null())
                .filter(not(active_detection_holds_file)),
        )
        .set(workspace_files::deleted_at.eq(diesel::dsl::now))
        .returning(WorkspaceFile::as_returning())
        .get_results(self)
        .await
        .map_err(Error::from)?;

        // Drop the import-origin rows of exactly the files just deleted, so
        // re-import is never blocked (see `delete_workspace_file`).
        let deleted_ids: Vec<Uuid> = deleted.iter().map(|file| file.id).collect();
        diesel::delete(
            workspace_file_imports::table
                .filter(workspace_file_imports::file_id.eq_any(&deleted_ids)),
        )
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};
    use uuid::Uuid;

    use super::*;
    use crate::PgConn;
    use crate::model::{NewWorkspaceConnection, NewWorkspaceDetection, NewWorkspaceFile};
    use crate::query::{
        WorkspaceConnectionRepository, WorkspaceDetectionRepository, WorkspaceFileRepository,
    };
    use crate::test_util::{TestDatabase, backdate};

    /// Creates a file that is already past its retention window: its `created_at`
    /// is two hours ago and its `expires_at` an hour after that (so the
    /// `expires_at >= created_at` check holds while `expires_at < now()`).
    async fn expired_file(
        conn: &mut PgConn,
        workspace_id: Uuid,
        account_id: Uuid,
    ) -> anyhow::Result<WorkspaceFile> {
        let created = Timestamp::now() - Span::new().hours(2);
        let file = conn
            .create_workspace_file(NewWorkspaceFile::test(workspace_id, account_id))
            .await?;
        // Backdate `created_at` and set `expires_at` an hour later, together, so
        // the file is expired against `now()` while `expires_at >= created_at`
        // still holds.
        backdate::file_span(conn, file.id, created, created + Span::new().hours(1)).await?;
        Ok(file)
    }

    #[tokio::test]
    async fn create_and_scoped_lookups_exclude_soft_deleted() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let file = conn
            .create_workspace_file(NewWorkspaceFile::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        assert!(conn.find_workspace_file_by_id(file.id).await?.is_some());
        assert!(
            conn.find_file_in_workspace(seeded.workspace_id, file.id)
                .await?
                .is_some()
        );
        assert!(
            conn.find_file_in_workspace_with_creator(seeded.workspace_id, file.id)
                .await?
                .is_some()
        );
        // Scoped to another workspace: not found.
        assert!(
            conn.find_file_in_workspace(Uuid::now_v7(), file.id)
                .await?
                .is_none()
        );

        // Soft delete hides it from every read.
        conn.delete_workspace_file(file.id).await?;
        assert!(conn.find_workspace_file_by_id(file.id).await?.is_none());
        assert!(
            conn.find_file_in_workspace(seeded.workspace_id, file.id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_only_documents_and_filters_by_extension_and_hash() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A pdf original document, a txt original, and an audit blob (not a document).
        let mut pdf = NewWorkspaceFile::test(seeded.workspace_id, seeded.account_id);
        pdf.file_extension = Some("pdf".to_owned());
        pdf.file_hash_sha256 = vec![7u8; 32];
        let pdf = conn.create_workspace_file(pdf).await?;
        let txt = conn
            .create_workspace_file(NewWorkspaceFile::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let mut audit = NewWorkspaceFile::test(seeded.workspace_id, seeded.account_id);
        audit.file_kind = Some(FileKind::Audit);
        let audit = conn.create_workspace_file(audit).await?;

        // The document listing excludes the audit blob.
        let all = conn
            .cursor_list_workspace_files(
                seeded.workspace_id,
                CursorPagination::new(50),
                FileFilter::default(),
            )
            .await?;
        let ids: Vec<_> = all.items.iter().map(|f| f.item.id).collect();
        assert!(ids.contains(&pdf.id) && ids.contains(&txt.id));
        assert!(!ids.contains(&audit.id), "audit blobs are not documents");

        // Extension filter narrows to the pdf.
        let pdfs = conn
            .cursor_list_workspace_files(
                seeded.workspace_id,
                CursorPagination::new(50),
                FileFilter {
                    extensions: Some(vec!["pdf".to_owned()]),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            pdfs.items.iter().map(|f| f.item.id).collect::<Vec<_>>(),
            vec![pdf.id]
        );

        // Exact-hash filter (dedup lookup) finds the pdf by its content hash.
        let by_hash = conn
            .cursor_list_workspace_files(
                seeded.workspace_id,
                CursorPagination::new(50),
                FileFilter {
                    hash: Some(vec![7u8; 32]),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            by_hash.items.iter().map(|f| f.item.id).collect::<Vec<_>>(),
            vec![pdf.id]
        );

        // A present-but-empty extension set matches nothing (an active facet).
        let none = conn
            .cursor_list_workspace_files(
                seeded.workspace_id,
                CursorPagination::new(50),
                FileFilter {
                    extensions: Some(vec![]),
                    ..Default::default()
                },
            )
            .await?;
        assert!(none.items.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn import_origin_round_trips_and_is_dropped_on_delete() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let connection = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        let file = conn
            .record_imported_file(
                NewWorkspaceFile::test(seeded.workspace_id, seeded.account_id),
                connection.id,
                "remote/key.pdf".to_owned(),
            )
            .await?;

        // The imported source key is reported for the connection.
        assert_eq!(
            conn.imported_keys_for_connection(connection.id).await?,
            vec!["remote/key.pdf".to_owned()]
        );
        assert_eq!(
            conn.imported_files_for_connection(connection.id)
                .await?
                .len(),
            1
        );

        // Deleting the file drops its import origin, so the key frees up for re-import.
        conn.delete_workspace_file(file.id).await?;
        assert!(
            conn.imported_keys_for_connection(connection.id)
                .await?
                .is_empty()
        );
        Ok(())
    }

    #[tokio::test]
    async fn redacted_files_not_exported_excludes_already_exported() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let connection = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // Two redacted files; one already exported to the connection.
        let mut a = NewWorkspaceFile::test(seeded.workspace_id, seeded.account_id);
        a.file_kind = Some(FileKind::Redacted);
        let a = conn.create_workspace_file(a).await?;
        let mut b = NewWorkspaceFile::test(seeded.workspace_id, seeded.account_id);
        b.file_kind = Some(FileKind::Redacted);
        let b = conn.create_workspace_file(b).await?;
        conn.record_exported_file(a.id, connection.id, "out/a.pdf".to_owned())
            .await?;

        // Only the not-yet-exported redacted file is returned.
        let pending = conn
            .redacted_files_not_exported(seeded.workspace_id, connection.id)
            .await?;
        assert_eq!(pending.iter().map(|f| f.id).collect::<Vec<_>>(), vec![b.id]);
        Ok(())
    }

    #[tokio::test]
    async fn expiry_sweep_holds_files_of_in_progress_detections() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        // A free expired file: eligible for the sweep.
        let free = expired_file(&mut conn, seeded.workspace_id, seeded.account_id).await?;

        // An expired file that is the input of a Pending detection.
        let held_file = expired_file(&mut conn, seeded.workspace_id, seeded.account_id).await?;
        let _detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.pipeline_id,
                seeded.account_id,
                held_file.id,
            ))
            .await?;

        // The sweep returns the free file but holds the in-progress detection's input.
        let due = conn.files_due_for_expiry(50).await?;
        let ids: Vec<_> = due.iter().map(|f| f.id).collect();
        assert!(ids.contains(&free.id));
        assert!(
            !ids.contains(&held_file.id),
            "an in-progress input is held back"
        );
        Ok(())
    }

    #[tokio::test]
    async fn purge_lifecycle_lists_then_stamps() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let file = conn
            .create_workspace_file(NewWorkspaceFile::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // A live file is not pending purge.
        assert!(
            !conn
                .files_pending_purge(50)
                .await?
                .iter()
                .any(|f| f.id == file.id)
        );

        // After a soft delete it is pending purge (object not yet reclaimed).
        conn.delete_workspace_file(file.id).await?;
        assert!(
            conn.files_pending_purge(50)
                .await?
                .iter()
                .any(|f| f.id == file.id)
        );

        // Marking it purged takes it out of the pending set.
        conn.mark_file_purged(file.id).await?;
        assert!(
            !conn
                .files_pending_purge(50)
                .await?
                .iter()
                .any(|f| f.id == file.id)
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_files_in_workspace_transitions_only_live_scoped_rows() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let a = conn
            .create_workspace_file(NewWorkspaceFile::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let b = conn
            .create_workspace_file(NewWorkspaceFile::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // Deleting [a, b, unknown] returns exactly the two live rows it changed.
        let deleted = conn
            .delete_files_in_workspace(seeded.workspace_id, &[a.id, b.id, Uuid::now_v7()])
            .await?;
        let mut deleted_ids: Vec<_> = deleted.iter().map(|f| f.id).collect();
        deleted_ids.sort();
        let mut expected = vec![a.id, b.id];
        expected.sort();
        assert_eq!(deleted_ids, expected);

        // A second call transitions nothing (already deleted).
        assert!(
            conn.delete_files_in_workspace(seeded.workspace_id, &[a.id, b.id])
                .await?
                .is_empty()
        );
        Ok(())
    }
}
