//! Document file repository for managing document file table operations.

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentFile, NewDocumentFile, UpdateDocumentFile};
use crate::types::{ProcessingStatus, VirusScanStatus};
use crate::{PgError, PgResult, schema};

/// Repository for document file table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct DocumentFileRepository;

impl DocumentFileRepository {
    /// Creates a new document file repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new document file.
    pub async fn create_document_file(
        conn: &mut AsyncPgConnection,
        new_file: NewDocumentFile,
    ) -> PgResult<DocumentFile> {
        use schema::document_files;

        diesel::insert_into(document_files::table)
            .values(&new_file)
            .returning(DocumentFile::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds a document file by its ID.
    pub async fn find_document_file_by_id(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
    ) -> PgResult<Option<DocumentFile>> {
        use schema::document_files::{self, dsl};

        document_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Lists files for a document.
    pub async fn list_document_files(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Lists files for an account.
    pub async fn list_account_files(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        document_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Updates a document file.
    pub async fn update_document_file(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        updates: UpdateDocumentFile,
    ) -> PgResult<DocumentFile> {
        use schema::document_files::{self, dsl};

        diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(&updates)
            .returning(DocumentFile::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Soft deletes a document file.
    pub async fn delete_document_file(conn: &mut AsyncPgConnection, file_id: Uuid) -> PgResult<()> {
        use schema::document_files::{self, dsl};

        diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    // Processing methods

    /// Gets files pending processing.
    pub async fn get_pending_files(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Pending))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::processing_priority.desc())
            .then_order_by(dsl::created_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Updates file processing status.
    pub async fn update_processing_status(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        status: ProcessingStatus,
        error: Option<String>,
        duration_ms: Option<i32>,
    ) -> PgResult<DocumentFile> {
        use schema::document_files::{self, dsl};

        let mut updates = UpdateDocumentFile {
            processing_status: Some(status),
            processing_error: error,
            processing_duration_ms: duration_ms,
            ..Default::default()
        };

        if status == ProcessingStatus::Completed {
            updates.processing_error = None;
        }

        diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set((
                &updates,
                dsl::processing_attempts.eq(dsl::processing_attempts + 1),
            ))
            .returning(DocumentFile::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Updates file processing scores.
    pub async fn update_processing_scores(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        processing_score: BigDecimal,
        completeness_score: BigDecimal,
        confidence_score: BigDecimal,
    ) -> PgResult<DocumentFile> {
        Self::update_document_file(
            conn,
            file_id,
            UpdateDocumentFile {
                processing_score: Some(processing_score),
                completeness_score: Some(completeness_score),
                confidence_score: Some(confidence_score),
                ..Default::default()
            },
        )
        .await
    }

    /// Updates virus scan status.
    pub async fn update_virus_scan_status(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        scan_status: VirusScanStatus,
    ) -> PgResult<DocumentFile> {
        Self::update_document_file(
            conn,
            file_id,
            UpdateDocumentFile {
                virus_scan_status: Some(scan_status),
                ..Default::default()
            },
        )
        .await
    }

    // Query methods

    /// Gets files by hash (for deduplication).
    pub async fn find_files_by_hash(
        conn: &mut AsyncPgConnection,
        file_hash: &[u8],
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        document_files::table
            .filter(dsl::file_hash_sha256.eq(file_hash))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds files by processing status.
    pub async fn find_files_by_status(
        conn: &mut AsyncPgConnection,
        status: ProcessingStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        document_files::table
            .filter(dsl::processing_status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds files by virus scan status.
    pub async fn find_files_by_scan_status(
        conn: &mut AsyncPgConnection,
        scan_status: VirusScanStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        document_files::table
            .filter(dsl::virus_scan_status.eq(scan_status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds files with processing failures.
    pub async fn find_failed_files(
        conn: &mut AsyncPgConnection,
        min_attempts: Option<i32>,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let mut query = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::processing_attempts.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .into_boxed();

        if let Some(attempts) = min_attempts {
            query = query.filter(dsl::processing_attempts.ge(attempts));
        }

        query.load(conn).await.map_err(PgError::from)
    }

    /// Finds large files above a certain size threshold.
    pub async fn find_large_files(
        conn: &mut AsyncPgConnection,
        min_size_bytes: i64,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        document_files::table
            .filter(dsl::file_size_bytes.ge(min_size_bytes))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::file_size_bytes.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    // Statistics and analytics

    /// Gets file statistics for a document.
    pub async fn get_document_file_stats(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<DocumentFileStats> {
        use schema::document_files::{self, dsl};

        // Total files
        let total_count: i64 = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Processing statuses
        let pending_count: i64 = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::processing_status.eq(ProcessingStatus::Pending))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let processing_count: i64 = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::processing_status.eq(ProcessingStatus::Processing))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let completed_count: i64 = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::processing_status.eq(ProcessingStatus::Completed))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let failed_count: i64 = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Total file size
        let total_size: i64 = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first::<Option<BigDecimal>>(conn)
            .await
            .map_err(PgError::from)?
            .map(|bd| bd.to_string().parse().unwrap_or(0))
            .unwrap_or(0);

        Ok(DocumentFileStats {
            total_count,
            pending_count,
            processing_count,
            completed_count,
            failed_count,
            total_size,
        })
    }

    /// Gets storage usage for an account.
    pub async fn get_user_storage_usage(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_files::{self, dsl};

        let total_size: i64 = document_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first::<Option<BigDecimal>>(conn)
            .await
            .map_err(PgError::from)?
            .map(|bd| bd.to_string().parse().unwrap_or(0))
            .unwrap_or(0);

        Ok(total_size)
    }

    /// Gets processing summary for all files.
    pub async fn get_processing_summary(
        conn: &mut AsyncPgConnection,
        document_id: Option<Uuid>,
    ) -> PgResult<ProcessingSummary> {
        use schema::document_files::{self, dsl};

        let mut pending_query = document_files::table
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::processing_status.eq(ProcessingStatus::Pending))
            .into_boxed();

        let mut processing_query = document_files::table
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::processing_status.eq(ProcessingStatus::Processing))
            .into_boxed();

        let mut completed_query = document_files::table
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::processing_status.eq(ProcessingStatus::Completed))
            .into_boxed();

        let mut failed_query = document_files::table
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
            .into_boxed();

        if let Some(doc_id) = document_id {
            pending_query = pending_query.filter(dsl::document_id.eq(doc_id));
            processing_query = processing_query.filter(dsl::document_id.eq(doc_id));
            completed_query = completed_query.filter(dsl::document_id.eq(doc_id));
            failed_query = failed_query.filter(dsl::document_id.eq(doc_id));
        }

        let pending: i64 = pending_query
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let processing: i64 = processing_query
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let completed: i64 = completed_query
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let failed: i64 = failed_query
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(ProcessingSummary {
            pending,
            processing,
            completed,
            failed,
        })
    }

    // Cleanup and maintenance

    /// Cleans up files marked for auto-deletion.
    pub async fn cleanup_auto_delete_files(conn: &mut AsyncPgConnection) -> PgResult<i64> {
        use schema::document_files::{self, dsl};

        diesel::update(
            document_files::table
                .filter(dsl::auto_delete_at.lt(OffsetDateTime::now_utc()))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }

    /// Resets failed processing jobs for retry.
    pub async fn reset_failed_processing(
        conn: &mut AsyncPgConnection,
        max_attempts: i32,
    ) -> PgResult<i64> {
        use schema::document_files::{self, dsl};

        diesel::update(
            document_files::table
                .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
                .filter(dsl::processing_attempts.lt(max_attempts))
                .filter(dsl::deleted_at.is_null()),
        )
        .set((
            dsl::processing_status.eq(ProcessingStatus::Pending),
            dsl::processing_error.eq(None::<String>),
        ))
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }

    /// Hard deletes old soft-deleted files.
    pub async fn purge_old_files(
        conn: &mut AsyncPgConnection,
        older_than_days: u32,
    ) -> PgResult<i64> {
        use schema::document_files::{self, dsl};

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(older_than_days as i64);

        diesel::delete(document_files::table.filter(dsl::deleted_at.lt(cutoff_date)))
            .execute(conn)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }
}

/// Statistics for document files.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentFileStats {
    /// Total number of files
    pub total_count: i64,
    /// Files pending processing
    pub pending_count: i64,
    /// Files currently being processed
    pub processing_count: i64,
    /// Successfully processed files
    pub completed_count: i64,
    /// Failed processing files
    pub failed_count: i64,
    /// Total file size in bytes
    pub total_size: i64,
}

impl DocumentFileStats {
    /// Returns the completion rate as a percentage (0-100).
    pub fn completion_rate(&self) -> f64 {
        if self.total_count == 0 {
            100.0
        } else {
            (self.completed_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Returns the failure rate as a percentage (0-100).
    pub fn failure_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            (self.failed_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Returns whether there are files still being processed.
    pub fn has_pending_work(&self) -> bool {
        self.pending_count > 0 || self.processing_count > 0
    }

    /// Returns whether there are failed files that need attention.
    pub fn has_failures(&self) -> bool {
        self.failed_count > 0
    }

    /// Returns average file size in bytes.
    pub fn average_file_size(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            self.total_size as f64 / self.total_count as f64
        }
    }
}

/// Processing summary statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessingSummary {
    /// Files pending processing
    pub pending: i64,
    /// Files currently being processed
    pub processing: i64,
    /// Successfully processed files
    pub completed: i64,
    /// Failed processing files
    pub failed: i64,
}

impl ProcessingSummary {
    /// Returns the total number of files.
    pub fn total(&self) -> i64 {
        self.pending + self.processing + self.completed + self.failed
    }

    /// Returns the completion rate as a percentage (0-100).
    pub fn completion_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            100.0
        } else {
            (self.completed as f64 / total as f64) * 100.0
        }
    }

    /// Returns whether the processing queue is active.
    pub fn is_active(&self) -> bool {
        self.pending > 0 || self.processing > 0
    }

    /// Returns whether there are processing failures.
    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }
}
