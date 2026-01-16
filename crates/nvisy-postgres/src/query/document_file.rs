//! Document files repository for managing uploaded document files.

use std::future::Future;

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use uuid::Uuid;

use crate::model::{DocumentFile, NewDocumentFile, UpdateDocumentFile};
use crate::types::{
    CursorPage, CursorPagination, FileFilter, FileSortBy, FileSortField, OffsetPagination,
    ProcessingStatus, SortOrder,
};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for document file database operations.
///
/// Handles file lifecycle management including upload tracking, processing
/// status updates, virus scanning, storage management, and cleanup operations.
pub trait DocumentFileRepository {
    /// Creates a new document file record.
    fn create_document_file(
        &mut self,
        new_file: NewDocumentFile,
    ) -> impl Future<Output = PgResult<DocumentFile>> + Send;

    /// Finds a file by its unique identifier.
    fn find_document_file_by_id(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentFile>>> + Send;

    /// Finds a file by ID within a specific workspace.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_workspace_file(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentFile>>> + Send;

    /// Lists all files associated with a document with offset pagination.
    fn offset_list_document_files(
        &mut self,
        document_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Lists all files uploaded by a specific account with offset pagination.
    fn offset_list_account_files(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Updates a file with new metadata or settings.
    fn update_document_file(
        &mut self,
        file_id: Uuid,
        updates: UpdateDocumentFile,
    ) -> impl Future<Output = PgResult<DocumentFile>> + Send;

    /// Soft deletes a file by setting the deletion timestamp.
    fn delete_document_file(&mut self, file_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Soft deletes multiple files in a workspace by setting deletion timestamps.
    ///
    /// Returns the number of files deleted.
    fn delete_document_files(
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
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Lists all files in a workspace with cursor pagination and optional filtering.
    fn cursor_list_workspace_files(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: FileFilter,
    ) -> impl Future<Output = PgResult<CursorPage<DocumentFile>>> + Send;

    /// Finds files with a matching SHA-256 hash.
    fn find_files_by_hash(
        &mut self,
        file_hash: &[u8],
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Finds files with a specific processing status.
    fn find_files_by_status(
        &mut self,
        status: ProcessingStatus,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Calculates total storage usage for an account.
    fn get_account_storage_usage(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<BigDecimal>> + Send;

    /// Finds multiple files by their IDs.
    fn find_document_files_by_ids(
        &mut self,
        file_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Lists all versions of a file (the file itself and all files that have it as parent).
    ///
    /// Returns files ordered by version_number descending (newest first).
    fn list_file_versions(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Finds the latest version of a file by traversing the version chain.
    ///
    /// Starting from a file, follows the chain of files where parent_id points
    /// to the previous version and returns the one with the highest version_number.
    fn find_latest_version(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentFile>>> + Send;

    /// Gets the next version number for creating a new version of a file.
    fn get_next_version_number(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<i32>> + Send;
}

impl DocumentFileRepository for PgConnection {
    async fn create_document_file(&mut self, new_file: NewDocumentFile) -> PgResult<DocumentFile> {
        use schema::document_files;

        let file = diesel::insert_into(document_files::table)
            .values(&new_file)
            .returning(DocumentFile::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn find_document_file_by_id(&mut self, file_id: Uuid) -> PgResult<Option<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let file = document_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn find_workspace_file(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> PgResult<Option<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let file = document_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn offset_list_document_files(
        &mut self,
        document_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn offset_list_account_files(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn update_document_file(
        &mut self,
        file_id: Uuid,
        updates: UpdateDocumentFile,
    ) -> PgResult<DocumentFile> {
        use schema::document_files::{self, dsl};

        let file = diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(&updates)
            .returning(DocumentFile::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn delete_document_file(&mut self, file_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::document_files::{self, dsl};

        diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn delete_document_files(
        &mut self,
        workspace_id: Uuid,
        file_ids: &[Uuid],
    ) -> PgResult<usize> {
        use diesel::dsl::now;
        use schema::document_files::{self, dsl};

        let count = diesel::update(
            document_files::table
                .filter(dsl::id.eq_any(file_ids))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(now))
        .execute(self)
        .await
        .map_err(PgError::from)?;

        Ok(count)
    }

    async fn offset_list_workspace_files(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: FileSortBy,
        filter: FileFilter,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        // Build base query
        let mut query = document_files::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply format filter using file extensions
        if !filter.is_empty() {
            let extensions: Vec<String> =
                filter.extensions().iter().map(|s| s.to_string()).collect();
            query = query.filter(dsl::file_extension.eq_any(extensions));
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
            .select(DocumentFile::as_select())
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
    ) -> PgResult<CursorPage<DocumentFile>> {
        use schema::document_files::{self, dsl};

        // Precompute filter values
        let search_term = filter.search_term().map(|s| s.to_string());
        let extensions: Vec<String> = filter.extensions().iter().map(|s| s.to_string()).collect();

        // Build base query with filters
        let mut base_query = document_files::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply trigram search filter (pg_trgm)
        if let Some(ref term) = search_term {
            base_query = base_query.filter(dsl::display_name.trgm_similar_to(term));
        }

        // Apply format filter using file extensions
        if !extensions.is_empty() {
            base_query = base_query.filter(dsl::file_extension.eq_any(&extensions));
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

        // Rebuild query for fetching items (can't reuse boxed query after count)
        let mut query = document_files::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply trigram search filter (pg_trgm)
        if let Some(ref term) = search_term {
            query = query.filter(dsl::display_name.trgm_similar_to(term));
        }

        // Apply format filter using file extensions
        if !extensions.is_empty() {
            query = query.filter(dsl::file_extension.eq_any(&extensions));
        }

        let limit = pagination.limit + 1;

        // Apply cursor filter if present
        let items: Vec<DocumentFile> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(DocumentFile::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select(DocumentFile::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |f: &DocumentFile| (f.created_at.into(), f.id),
        ))
    }

    async fn find_files_by_hash(&mut self, file_hash: &[u8]) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::file_hash_sha256.eq(file_hash))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_files_by_status(
        &mut self,
        status: ProcessingStatus,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::processing_status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn get_account_storage_usage(&mut self, account_id: Uuid) -> PgResult<BigDecimal> {
        use schema::document_files::{self, dsl};

        let usage: Option<BigDecimal> = document_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first(self)
            .await
            .map_err(PgError::from)?;

        Ok(usage.unwrap_or_else(|| BigDecimal::from(0)))
    }

    async fn find_document_files_by_ids(
        &mut self,
        file_ids: &[Uuid],
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::id.eq_any(file_ids))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn list_file_versions(&mut self, file_id: Uuid) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        // Get the original file and all files that have it (or its descendants) as parent
        // This query gets the file itself plus all files where parent_id = file_id
        let files = document_files::table
            .filter(dsl::id.eq(file_id).or(dsl::parent_id.eq(file_id)))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::version_number.desc())
            .select(DocumentFile::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_latest_version(&mut self, file_id: Uuid) -> PgResult<Option<DocumentFile>> {
        use schema::document_files::{self, dsl};

        // Find the file with highest version_number that has file_id as parent,
        // or the file itself if no newer versions exist
        let latest = document_files::table
            .filter(dsl::id.eq(file_id).or(dsl::parent_id.eq(file_id)))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::version_number.desc())
            .select(DocumentFile::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(latest)
    }

    async fn get_next_version_number(&mut self, file_id: Uuid) -> PgResult<i32> {
        use diesel::dsl::max;
        use schema::document_files::{self, dsl};

        // Get the max version_number from the file and its versions
        let max_version: Option<i32> = document_files::table
            .filter(dsl::id.eq(file_id).or(dsl::parent_id.eq(file_id)))
            .filter(dsl::deleted_at.is_null())
            .select(max(dsl::version_number))
            .first(self)
            .await
            .map_err(PgError::from)?;

        Ok(max_version.unwrap_or(0) + 1)
    }
}
