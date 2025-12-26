//! Document files repository for managing uploaded document files.

use std::future::Future;

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentFile, NewDocumentFile, UpdateDocumentFile};
use crate::types::{ProcessingStatus, VirusScanStatus};
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for document file database operations.
///
/// Handles file lifecycle management including upload tracking, processing
/// status updates, virus scanning, storage management, and cleanup operations.
pub trait DocumentFileRepository {
    /// Creates a new document file record.
    fn create_document_file(
        &self,
        new_file: NewDocumentFile,
    ) -> impl Future<Output = PgResult<DocumentFile>> + Send;

    /// Finds a file by its unique identifier.
    fn find_document_file_by_id(
        &self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentFile>>> + Send;

    /// Finds a file by ID within a specific project.
    ///
    /// Provides project-scoped access control at the database level.
    fn find_project_file(
        &self,
        project_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentFile>>> + Send;

    /// Lists all files associated with a document.
    fn list_document_files(
        &self,
        document_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Lists all files uploaded by a specific account.
    fn list_account_files(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Updates a file with new metadata or settings.
    fn update_document_file(
        &self,
        file_id: Uuid,
        updates: UpdateDocumentFile,
    ) -> impl Future<Output = PgResult<DocumentFile>> + Send;

    /// Soft deletes a file by setting the deletion timestamp.
    fn delete_document_file(&self, file_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Retrieves files awaiting processing.
    ///
    /// Returns files ordered by priority and creation time.
    fn get_pending_files(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Updates the processing status of a file.
    fn update_processing_status(
        &self,
        file_id: Uuid,
        status: ProcessingStatus,
    ) -> impl Future<Output = PgResult<DocumentFile>> + Send;

    /// Updates the virus scan status of a file.
    fn update_virus_scan_status(
        &self,
        file_id: Uuid,
        scan_status: VirusScanStatus,
    ) -> impl Future<Output = PgResult<DocumentFile>> + Send;

    /// Finds multiple files by their IDs in a single query.
    fn find_document_files_by_ids(
        &self,
        file_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Finds all files belonging to a specific project.
    fn find_document_files_by_project(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Finds files with a matching SHA-256 hash.
    fn find_files_by_hash(
        &self,
        file_hash: &[u8],
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Finds files with a specific processing status.
    fn find_files_by_status(
        &self,
        status: ProcessingStatus,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Finds files with a specific virus scan status.
    fn find_files_by_scan_status(
        &self,
        scan_status: VirusScanStatus,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Finds files that failed processing.
    fn find_failed_files(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Finds files exceeding a size threshold.
    fn find_large_files(
        &self,
        size_threshold: i64,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentFile>>> + Send;

    /// Calculates total storage usage for an account.
    fn get_user_storage_usage(
        &self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<BigDecimal>> + Send;

    /// Soft deletes files past their auto-delete timestamp.
    ///
    /// Returns the count of affected files.
    fn cleanup_auto_delete_files(&self) -> impl Future<Output = PgResult<usize>> + Send;

    /// Resets failed files to pending status for reprocessing.
    ///
    /// Returns the count of affected files.
    fn reset_failed_processing(
        &self,
        file_ids: Vec<Uuid>,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    /// Soft deletes files older than the retention period.
    ///
    /// Returns the count of affected files.
    fn purge_old_files(&self, retention_days: i32) -> impl Future<Output = PgResult<usize>> + Send;
}

impl DocumentFileRepository for PgClient {
    async fn create_document_file(&self, new_file: NewDocumentFile) -> PgResult<DocumentFile> {
        let mut conn = self.get_connection().await?;

        use schema::document_files;

        let file = diesel::insert_into(document_files::table)
            .values(&new_file)
            .returning(DocumentFile::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn find_document_file_by_id(&self, file_id: Uuid) -> PgResult<Option<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let file = document_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn find_project_file(
        &self,
        project_id: Uuid,
        file_id: Uuid,
    ) -> PgResult<Option<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let file = document_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn list_document_files(
        &self,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn list_account_files(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn update_document_file(
        &self,
        file_id: Uuid,
        updates: UpdateDocumentFile,
    ) -> PgResult<DocumentFile> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let file = diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(&updates)
            .returning(DocumentFile::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn delete_document_file(&self, file_id: Uuid) -> PgResult<()> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn get_pending_files(&self, pagination: Pagination) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Pending))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::processing_priority.desc())
            .then_order_by(dsl::created_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn update_processing_status(
        &self,
        file_id: Uuid,
        status: ProcessingStatus,
    ) -> PgResult<DocumentFile> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let updates = UpdateDocumentFile {
            processing_status: Some(status),
            ..Default::default()
        };

        let file = diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(&updates)
            .returning(DocumentFile::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    async fn update_virus_scan_status(
        &self,
        file_id: Uuid,
        scan_status: VirusScanStatus,
    ) -> PgResult<DocumentFile> {
        self.update_document_file(
            file_id,
            UpdateDocumentFile {
                virus_scan_status: Some(scan_status),
                ..Default::default()
            },
        )
        .await
    }

    async fn find_document_files_by_ids(&self, file_ids: &[Uuid]) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::id.eq_any(file_ids))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_document_files_by_project(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_files_by_hash(&self, file_hash: &[u8]) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::file_hash_sha256.eq(file_hash))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_files_by_status(
        &self,
        status: ProcessingStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::processing_status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_files_by_scan_status(
        &self,
        scan_status: VirusScanStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::virus_scan_status.eq(scan_status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_failed_files(&self, pagination: Pagination) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn find_large_files(
        &self,
        size_threshold: i64,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::file_size_bytes.gt(size_threshold))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::file_size_bytes.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    async fn get_user_storage_usage(&self, account_id: Uuid) -> PgResult<BigDecimal> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let usage: Option<BigDecimal> = document_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(usage.unwrap_or_else(|| BigDecimal::from(0)))
    }

    async fn cleanup_auto_delete_files(&self) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let affected = diesel::update(document_files::table)
            .filter(dsl::auto_delete_at.le(jiff_diesel::Timestamp::from(Timestamp::now())))
            .filter(dsl::deleted_at.is_null())
            .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    async fn reset_failed_processing(&self, file_ids: Vec<Uuid>) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let affected = diesel::update(document_files::table)
            .filter(dsl::id.eq_any(file_ids))
            .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
            .set(dsl::processing_status.eq(ProcessingStatus::Pending))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    async fn purge_old_files(&self, retention_days: i32) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::document_files::{self, dsl};

        let cutoff_date = jiff_diesel::Timestamp::from(
            Timestamp::now() - Span::new().days(retention_days as i64),
        );

        let affected = diesel::update(document_files::table)
            .filter(dsl::created_at.lt(cutoff_date))
            .filter(dsl::deleted_at.is_null())
            .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }
}
