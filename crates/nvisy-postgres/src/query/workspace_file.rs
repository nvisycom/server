//! Workspace files repository for managing uploaded files.

use std::future::Future;

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use uuid::Uuid;

use crate::model::{NewWorkspaceFile, NewWorkspaceFileImport, UpdateWorkspaceFile, WorkspaceFile};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, FileFilter, FileKind, FileSortBy, FileSortField,
    OffsetPagination, PipelineRunStatus, SortOrder, WithAccountRef,
};
use crate::{PgConnection, PgError, PgResult, schema};

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
    ) -> impl Future<Output = PgResult<WorkspaceFile>> + Send;

    /// Creates a workspace file and records its import origin (the connection and
    /// remote object key it came from) in a single transaction.
    fn record_imported_file(
        &mut self,
        new_file: NewWorkspaceFile,
        connection_id: Uuid,
        source_key: String,
    ) -> impl Future<Output = PgResult<WorkspaceFile>> + Send;

    /// Finds a workspace file by its unique identifier.
    fn find_workspace_file_by_id(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceFile>>> + Send;

    /// Finds a file by ID within a specific workspace.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_file_in_workspace(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceFile>>> + Send;

    /// Finds a file by id within a workspace, with the handle and avatar of the
    /// account that uploaded it.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_file_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WithAccountRef<WorkspaceFile>>>> + Send;

    /// Returns the remote object keys already imported (live) from a connection.
    ///
    /// Used to skip re-importing objects during a connection sync.
    fn imported_keys_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<String>>> + Send;

    /// Returns each live file imported from a connection, for deletion
    /// reconciliation (a source key absent from the remote listing identifies a
    /// removed source object).
    fn imported_files_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ImportedFileRef>>> + Send;

    /// Returns up to `limit` live files whose retention window has elapsed
    /// (`expires_at < now`). The data-retention worker sweeps these, purges their
    /// objects, and soft-deletes the rows.
    fn files_due_for_expiry(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<ExpiredFileRef>>> + Send;

    /// Recomputes `expires_at` for live files of `kind` in `workspace_id`,
    /// returning the number updated. Used to backfill when retention settings
    /// change. `None` clears the expiry (retention became `Forever`).
    fn backfill_files_expiry(
        &mut self,
        workspace_id: Uuid,
        kind: FileKind,
        expires_at: Option<jiff::Timestamp>,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Recomputes `expires_at` for live files of `kind` produced by a specific
    /// pipeline's runs (redacted outputs via `output_file_id`, audit blobs via
    /// `audit_file_id`), returning the number updated. Used to backfill when a
    /// pipeline's own retention override changes, without touching other
    /// pipelines' files. `None` clears the expiry.
    fn backfill_pipeline_files_expiry(
        &mut self,
        pipeline_id: Uuid,
        kind: FileKind,
        expires_at: Option<jiff::Timestamp>,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Lists all files uploaded by a specific account with offset pagination.
    fn offset_list_account_files(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceFile>>> + Send;

    /// Updates a workspace file with new metadata or settings.
    fn update_workspace_file(
        &mut self,
        file_id: Uuid,
        updates: UpdateWorkspaceFile,
    ) -> impl Future<Output = PgResult<WorkspaceFile>> + Send;

    /// Soft deletes a workspace file by setting the deletion timestamp.
    fn delete_workspace_file(&mut self, file_id: Uuid)
    -> impl Future<Output = PgResult<()>> + Send;

    /// Soft deletes multiple workspace files by setting deletion timestamps.
    ///
    /// Returns the number of files deleted.
    fn delete_workspace_files(
        &mut self,
        workspace_id: Uuid,
        file_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Lists all files in a workspace with sorting and filtering options.
    ///
    /// Supports filtering by file format and sorting by name, date, or size.
    fn offset_list_workspace_files(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: FileSortBy,
        filter: FileFilter,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceFile>>> + Send;

    /// Lists all files in a workspace with cursor pagination and optional
    /// filtering, each paired with the handle and avatar of the account that
    /// uploaded it.
    fn cursor_list_workspace_files(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: FileFilter,
    ) -> impl Future<Output = PgResult<CursorPage<WithAccountRef<WorkspaceFile>>>> + Send;

    /// Finds workspace files with a matching SHA-256 hash.
    fn find_workspace_files_by_hash(
        &mut self,
        file_hash: &[u8],
    ) -> impl Future<Output = PgResult<Vec<WorkspaceFile>>> + Send;

    /// Calculates total storage usage for an account.
    fn get_account_storage_usage(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<BigDecimal>> + Send;

    /// Finds multiple workspace files by their IDs.
    fn find_workspace_files_by_ids(
        &mut self,
        file_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<Vec<WorkspaceFile>>> + Send;

    /// Lists all versions of a file (the file itself and all files that have it as parent).
    ///
    /// Returns files ordered by version_number descending (newest first).
    fn list_workspace_file_versions(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceFile>>> + Send;

    /// Finds the latest version of a file by traversing the version chain.
    ///
    /// Starting from a file, follows the chain of files where parent_id points
    /// to the previous version and returns the one with the highest version_number.
    fn find_latest_workspace_file_version(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceFile>>> + Send;

    /// Gets the next version number for creating a new version of a file.
    fn get_next_workspace_file_version_number(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<i32>> + Send;
}

impl WorkspaceFileRepository for PgConnection {
    async fn create_workspace_file(
        &mut self,
        new_file: NewWorkspaceFile,
    ) -> PgResult<WorkspaceFile> {
        use schema::workspace_files;

        let file = diesel::insert_into(workspace_files::table)
            .values(&new_file)
            .returning(WorkspaceFile::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn record_imported_file(
        &mut self,
        new_file: NewWorkspaceFile,
        connection_id: Uuid,
        source_key: String,
    ) -> PgResult<WorkspaceFile> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_file_imports, workspace_files};

        self.transaction(async |conn| {
            let file = diesel::insert_into(workspace_files::table)
                .values(&new_file)
                .returning(WorkspaceFile::as_returning())
                .get_result(conn)
                .await
                .map_err(PgError::from)?;

            diesel::insert_into(workspace_file_imports::table)
                .values(NewWorkspaceFileImport {
                    file_id: file.id,
                    connection_id,
                    source_key,
                })
                .execute(conn)
                .await
                .map_err(PgError::from)?;

            Ok::<_, PgError>(file)
        })
        .await
    }

    async fn find_workspace_file_by_id(
        &mut self,
        file_id: Uuid,
    ) -> PgResult<Option<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        let file = workspace_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceFile::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn find_file_in_workspace(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> PgResult<Option<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        let file = workspace_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceFile::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn imported_keys_for_connection(&mut self, connection_id: Uuid) -> PgResult<Vec<String>> {
        use schema::{workspace_file_imports, workspace_files};

        let keys = workspace_file_imports::table
            .inner_join(workspace_files::table)
            .filter(workspace_file_imports::connection_id.eq(connection_id))
            .filter(workspace_files::deleted_at.is_null())
            .select(workspace_file_imports::source_key)
            .load::<String>(self)
            .await
            .map_err(PgError::from)?;

        Ok(keys)
    }

    async fn imported_files_for_connection(
        &mut self,
        connection_id: Uuid,
    ) -> PgResult<Vec<ImportedFileRef>> {
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
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn files_due_for_expiry(&mut self, limit: i64) -> PgResult<Vec<ExpiredFileRef>> {
        use diesel::dsl::{exists, not, now};
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::{workspace_files, workspace_pipeline_runs};

        // A run that has not finished (running or awaiting redaction) still needs
        // its input document and audit blob, so those files are held back from
        // expiry until the run reaches a terminal state. Otherwise an in-flight
        // detect/redact could lose its source or analysis mid-flight and get
        // stuck. Produced outputs are not protected — they only exist once a run
        // has completed.
        let active_run_holds_file = exists(
            workspace_pipeline_runs::table.filter(
                runs::status
                    .eq_any([
                        PipelineRunStatus::Queued,
                        PipelineRunStatus::Analyzing,
                        PipelineRunStatus::Analyzed,
                    ])
                    .and(
                        runs::input_file_id
                            .eq(workspace_files::id)
                            .or(runs::audit_file_id.eq(workspace_files::id.nullable())),
                    ),
            ),
        );

        let files = workspace_files::table
            .filter(workspace_files::expires_at.is_not_null())
            .filter(workspace_files::expires_at.lt(now))
            .filter(workspace_files::deleted_at.is_null())
            .filter(not(active_run_holds_file))
            .select((
                workspace_files::id,
                workspace_files::storage_path,
                workspace_files::storage_bucket,
            ))
            .limit(limit)
            .load::<ExpiredFileRef>(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn backfill_files_expiry(
        &mut self,
        workspace_id: Uuid,
        kind: FileKind,
        expires_at: Option<jiff::Timestamp>,
    ) -> PgResult<usize> {
        use schema::workspace_files::{self, dsl};

        let expires_at = expires_at.map(jiff_diesel::Timestamp::from);

        let count = diesel::update(workspace_files::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::file_kind.eq(kind))
            .filter(dsl::deleted_at.is_null())
            .set(dsl::expires_at.eq(expires_at))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn backfill_pipeline_files_expiry(
        &mut self,
        pipeline_id: Uuid,
        kind: FileKind,
        expires_at: Option<jiff::Timestamp>,
    ) -> PgResult<usize> {
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::{workspace_files, workspace_pipeline_runs};

        let expires_at = expires_at.map(jiff_diesel::Timestamp::from);

        // Collect the ids of files this pipeline's runs produced for `kind`:
        // redacted files are the runs' outputs, audit files their audit blobs.
        // Any other kind is not pipeline-produced, so there is nothing to do.
        let file_ids: Vec<Uuid> = match kind {
            FileKind::Redacted => workspace_pipeline_runs::table
                .filter(runs::pipeline_id.eq(pipeline_id))
                .filter(runs::output_file_id.is_not_null())
                .select(runs::output_file_id.assume_not_null())
                .load(self)
                .await
                .map_err(PgError::from)?,
            FileKind::Audit => workspace_pipeline_runs::table
                .filter(runs::pipeline_id.eq(pipeline_id))
                .filter(runs::audit_file_id.is_not_null())
                .select(runs::audit_file_id.assume_not_null())
                .load(self)
                .await
                .map_err(PgError::from)?,
            _ => return Ok(0),
        };

        if file_ids.is_empty() {
            return Ok(0);
        }

        let count = diesel::update(workspace_files::table)
            .filter(workspace_files::deleted_at.is_null())
            .filter(workspace_files::id.eq_any(file_ids))
            .set(workspace_files::expires_at.eq(expires_at))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn find_file_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> PgResult<Option<WithAccountRef<WorkspaceFile>>> {
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
            .map_err(PgError::from)?;

        Ok(row.map(|(item, account)| WithAccountRef { item, account }))
    }

    async fn offset_list_account_files(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        let files = workspace_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn update_workspace_file(
        &mut self,
        file_id: Uuid,
        updates: UpdateWorkspaceFile,
    ) -> PgResult<WorkspaceFile> {
        use schema::workspace_files::{self, dsl};

        let file = diesel::update(workspace_files::table.filter(dsl::id.eq(file_id)))
            .set(&updates)
            .returning(WorkspaceFile::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn delete_workspace_file(&mut self, file_id: Uuid) -> PgResult<()> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_file_imports, workspace_files};

        // Soft-delete the file and drop its import-origin row (if any) atomically.
        // The origin's `(connection_id, source_key)` uniqueness would otherwise
        // block ever re-importing that source object, since the soft delete keeps
        // the file row and its `ON DELETE CASCADE` never fires.
        self.transaction(async |conn| {
            diesel::update(workspace_files::table.filter(workspace_files::id.eq(file_id)))
                .set(workspace_files::deleted_at.eq(diesel::dsl::now))
                .execute(conn)
                .await
                .map_err(PgError::from)?;

            diesel::delete(
                workspace_file_imports::table.filter(workspace_file_imports::file_id.eq(file_id)),
            )
            .execute(conn)
            .await
            .map_err(PgError::from)?;

            Ok::<_, PgError>(())
        })
        .await
    }

    async fn delete_workspace_files(
        &mut self,
        workspace_id: Uuid,
        file_ids: &[Uuid],
    ) -> PgResult<usize> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_file_imports, workspace_files};

        let ids = file_ids.to_vec();
        self.transaction(async |conn| {
            let count = diesel::update(
                workspace_files::table
                    .filter(workspace_files::id.eq_any(&ids))
                    .filter(workspace_files::workspace_id.eq(workspace_id))
                    .filter(workspace_files::deleted_at.is_null()),
            )
            .set(workspace_files::deleted_at.eq(diesel::dsl::now))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

            // Drop import-origin rows so re-import is never blocked (see
            // `delete_workspace_file`).
            diesel::delete(
                workspace_file_imports::table.filter(workspace_file_imports::file_id.eq_any(&ids)),
            )
            .execute(conn)
            .await
            .map_err(PgError::from)?;

            Ok::<_, PgError>(count)
        })
        .await
    }

    async fn offset_list_workspace_files(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: FileSortBy,
        filter: FileFilter,
    ) -> PgResult<Vec<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        // Build base query
        let mut query = workspace_files::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::file_kind.eq_any(FileKind::DOCUMENTS))
            .into_boxed();

        // Apply the extension constraint. A present-but-empty set matches
        // nothing (an active facet with no members), so apply whenever `Some`.
        if let Some(extensions) = filter.extensions() {
            query = query.filter(dsl::file_extension.eq_any(extensions.to_vec()));
        }

        // Apply sorting
        let query = match (sort_by.field, sort_by.order) {
            (FileSortField::Name, SortOrder::Asc) => query.order(dsl::display_name.asc()),
            (FileSortField::Name, SortOrder::Desc) => query.order(dsl::display_name.desc()),
            (FileSortField::Date, SortOrder::Asc) => query.order(dsl::created_at.asc()),
            (FileSortField::Date, SortOrder::Desc) => query.order(dsl::created_at.desc()),
            (FileSortField::Size, SortOrder::Asc) => query.order(dsl::file_size_bytes.asc()),
            (FileSortField::Size, SortOrder::Desc) => query.order(dsl::file_size_bytes.desc()),
        };

        let files = query
            .select(WorkspaceFile::as_select())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn cursor_list_workspace_files(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: FileFilter,
    ) -> PgResult<CursorPage<WithAccountRef<WorkspaceFile>>> {
        use schema::workspace_files::dsl;
        use schema::{accounts, workspace_files};

        // Precompute filter values
        let search_term = filter.search_term().map(|s| s.to_string());
        let extensions: Option<Vec<String>> = filter.extensions().map(|e| e.to_vec());

        // Build base query with filters
        let mut base_query = workspace_files::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::file_kind.eq_any(FileKind::DOCUMENTS))
            .into_boxed();

        // Apply trigram search filter (pg_trgm)
        if let Some(ref term) = search_term {
            base_query = base_query.filter(dsl::display_name.trgm_similar_to(term));
        }

        // Apply the extension constraint. A present-but-empty set matches
        // nothing (an active facet with no members), so apply whenever `Some`.
        if let Some(ref extensions) = extensions {
            base_query = base_query.filter(dsl::file_extension.eq_any(extensions));
        }

        let total = if pagination.include_count {
            Some(
                base_query
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Rebuild query for fetching items (can't reuse boxed query after count).
        // The document-kind filter must be reapplied here too: it is what keeps
        // audit/artifact rows out of the list, and omitting it leaks them into
        // the results even though the count above excludes them.
        let mut query = workspace_files::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::file_kind.eq_any(FileKind::DOCUMENTS))
            .into_boxed();

        // Apply trigram search filter (pg_trgm)
        if let Some(ref term) = search_term {
            query = query.filter(dsl::display_name.trgm_similar_to(term));
        }

        // Apply the extension constraint. A present-but-empty set matches
        // nothing (an active facet with no members), so apply whenever `Some`.
        if let Some(ref extensions) = extensions {
            query = query.filter(dsl::file_extension.eq_any(extensions));
        }

        let limit = pagination.fetch_limit();

        // Apply cursor filter if present
        let rows: Vec<(WorkspaceFile, AccountRefRow)> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select((
                    WorkspaceFile::as_select(),
                    (
                        accounts::username,
                        accounts::display_name,
                        accounts::avatar_url,
                    ),
                ))
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select((
                    WorkspaceFile::as_select(),
                    (
                        accounts::username,
                        accounts::display_name,
                        accounts::avatar_url,
                    ),
                ))
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        let items: Vec<WithAccountRef<WorkspaceFile>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            (wc.item.created_at.into(), wc.item.id)
        }))
    }

    async fn find_workspace_files_by_hash(
        &mut self,
        file_hash: &[u8],
    ) -> PgResult<Vec<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        let files = workspace_files::table
            .filter(dsl::file_hash_sha256.eq(file_hash))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn get_account_storage_usage(&mut self, account_id: Uuid) -> PgResult<BigDecimal> {
        use schema::workspace_files::{self, dsl};

        let usage: Option<BigDecimal> = workspace_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first(self)
            .await
            .map_err(PgError::from)?;

        Ok(usage.unwrap_or_else(|| BigDecimal::from(0)))
    }

    async fn find_workspace_files_by_ids(
        &mut self,
        file_ids: &[Uuid],
    ) -> PgResult<Vec<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        let files = workspace_files::table
            .filter(dsl::id.eq_any(file_ids))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn list_workspace_file_versions(
        &mut self,
        file_id: Uuid,
    ) -> PgResult<Vec<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        // Get the original file and all files that have it (or its descendants) as parent
        // This query gets the file itself plus all files where parent_id = file_id
        let files = workspace_files::table
            .filter(dsl::id.eq(file_id).or(dsl::parent_id.eq(file_id)))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::version_number.desc())
            .select(WorkspaceFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_latest_workspace_file_version(
        &mut self,
        file_id: Uuid,
    ) -> PgResult<Option<WorkspaceFile>> {
        use schema::workspace_files::{self, dsl};

        // Find the file with highest version_number that has file_id as parent,
        // or the file itself if no newer versions exist
        let latest = workspace_files::table
            .filter(dsl::id.eq(file_id).or(dsl::parent_id.eq(file_id)))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::version_number.desc())
            .select(WorkspaceFile::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(latest)
    }

    async fn get_next_workspace_file_version_number(&mut self, file_id: Uuid) -> PgResult<i32> {
        use diesel::dsl::max;
        use schema::workspace_files::{self, dsl};

        // Get the max version_number from the file and its versions
        let max_version: Option<i32> = workspace_files::table
            .filter(dsl::id.eq(file_id).or(dsl::parent_id.eq(file_id)))
            .filter(dsl::deleted_at.is_null())
            .select(max(dsl::version_number))
            .first(self)
            .await
            .map_err(PgError::from)?;

        Ok(max_version.unwrap_or(0) + 1)
    }
}
