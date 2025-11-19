//! Document files repository for managing file storage and processing operations.

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentFile, NewDocumentFile, UpdateDocumentFile};
use crate::types::{ProcessingStatus, VirusScanStatus};
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive document file database operations.
///
/// Provides database operations for managing document files throughout their
/// lifecycle, including upload processing, virus scanning, deduplication,
/// storage management, and analytics. This repository handles all database
/// interactions related to file attachment management within document
/// workflows, serving as the primary interface for binary content storage.
///
/// The repository supports file processing pipelines, security scanning,
/// storage optimization through deduplication, and comprehensive monitoring
/// capabilities to enable reliable and secure file management experiences.
/// Files are treated as immutable content attachments that enhance document
/// collaboration through rich media support and version-controlled storage.
#[derive(Debug, Default, Clone, Copy)]
pub struct DocumentFileRepository;

impl DocumentFileRepository {
    /// Creates a new document file repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `DocumentFileRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new document file record in the database with processing setup.
    ///
    /// Initializes a new file record within the document storage system with
    /// metadata, processing status, and security scanning configuration. The file
    /// record is immediately queued for processing and security validation,
    /// enabling secure and reliable file attachment workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `new_file` - Complete file metadata including storage references and processing settings
    ///
    /// # Returns
    ///
    /// The created `DocumentFile` with database-generated ID and timestamps,
    /// or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - File becomes queued for processing and security scanning
    /// - Enables rich media attachment capabilities for documents
    /// - Supports collaborative workflows with file sharing
    /// - Creates audit trail for content security and compliance
    /// - Enables deduplication and storage optimization strategies
    pub async fn create_document_file(
        conn: &mut AsyncPgConnection,
        new_file: NewDocumentFile,
    ) -> PgResult<DocumentFile> {
        use schema::document_files;

        let file = diesel::insert_into(document_files::table)
            .values(&new_file)
            .returning(DocumentFile::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    /// Finds a document file by its unique identifier.
    ///
    /// Retrieves a specific file record using its UUID, automatically excluding
    /// soft-deleted files. This is the primary method for accessing individual
    /// file records for download, processing status checks, and metadata
    /// operations within document workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `file_id` - UUID of the file to retrieve
    ///
    /// # Returns
    ///
    /// The matching `DocumentFile` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
    pub async fn find_document_file_by_id(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
    ) -> PgResult<Option<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let file = document_files::table
            .filter(dsl::id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(file)
    }

    /// Lists files associated with a specific document.
    ///
    /// Retrieves a paginated list of files attached to a document,
    /// ordered by most recently created first. This supports document
    /// file management interfaces and provides teams with comprehensive
    /// visibility into document attachments and media content.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document whose files to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentFile` entries for the document, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    pub async fn list_document_files(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    /// Lists files uploaded by a specific account.
    ///
    /// Retrieves a paginated list of files uploaded by a user account,
    /// ordered by most recently created first. This supports user file
    /// management dashboards and provides individuals with comprehensive
    /// visibility into their file upload history and storage usage.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account whose files to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentFile` entries uploaded by the account, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    pub async fn list_account_files(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    /// Updates a document file with new metadata and processing information.
    ///
    /// Applies partial updates to an existing file record using the provided
    /// update structure. Only fields set to `Some(value)` will be modified,
    /// while `None` fields remain unchanged. This supports processing status
    /// updates, metadata enrichment, and administrative file management.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `file_id` - UUID of the file to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `DocumentFile` with new values and timestamp,
    /// or a database error if the operation fails.
    pub async fn update_document_file(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        updates: UpdateDocumentFile,
    ) -> PgResult<DocumentFile> {
        use schema::document_files::{self, dsl};

        let file = diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(&updates)
            .returning(DocumentFile::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    /// Soft deletes a document file by setting the deletion timestamp.
    ///
    /// Marks a file as deleted without permanently removing it from the
    /// database. This preserves file metadata for audit purposes and
    /// compliance requirements while preventing the file from appearing
    /// in normal file listings and download operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `file_id` - UUID of the file to soft delete
    ///
    /// # Returns
    ///
    /// `()` on successful deletion, or a database error if the operation fails.
    ///
    /// # Important Considerations
    ///
    /// Physical file storage cleanup should be handled separately through
    /// appropriate cleanup processes to ensure storage optimization while
    /// maintaining audit compliance requirements.
    pub async fn delete_document_file(conn: &mut AsyncPgConnection, file_id: Uuid) -> PgResult<()> {
        use schema::document_files::{self, dsl};

        diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Gets files pending processing with priority ordering.
    ///
    /// Retrieves files queued for processing, ordered by processing priority
    /// and creation time to ensure efficient processing queue management.
    /// This supports background processing workflows and enables reliable
    /// file processing pipeline coordination.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentFile` entries pending processing, ordered by
    /// priority (highest first) then creation time (oldest first),
    /// or a database error if the query fails.
    pub async fn get_pending_files(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Pending))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::processing_priority.desc())
            .then_order_by(dsl::created_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    /// Updates the processing status of a file.
    ///
    /// Modifies the processing status of a file to track its progress through
    /// the file processing pipeline. This enables status monitoring, queue
    /// management, and processing workflow coordination for reliable file
    /// handling operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `file_id` - UUID of the file to update
    /// * `status` - New processing status to set
    ///
    /// # Returns
    ///
    /// The updated `DocumentFile` with new processing status,
    /// or a database error if the operation fails.
    pub async fn update_processing_status(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        status: ProcessingStatus,
    ) -> PgResult<DocumentFile> {
        use schema::document_files::{self, dsl};

        let updates = UpdateDocumentFile {
            processing_status: Some(status),
            ..Default::default()
        };

        let file = diesel::update(document_files::table.filter(dsl::id.eq(file_id)))
            .set(&updates)
            .returning(DocumentFile::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(file)
    }

    /// Updates the virus scan status of a file.
    ///
    /// Modifies the virus scan status of a file to track security scanning
    /// progress and results. This enables security policy enforcement,
    /// threat detection workflows, and safe file access controls within
    /// the collaborative environment.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `file_id` - UUID of the file to update
    /// * `scan_status` - New virus scan status to set
    ///
    /// # Returns
    ///
    /// The updated `DocumentFile` with new scan status,
    /// or a database error if the operation fails.
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

    /// Finds files by their SHA256 hash for deduplication purposes.
    ///
    /// Retrieves files with matching content hashes to support deduplication
    /// strategies, storage optimization, and content integrity verification.
    /// This enables efficient storage management by identifying duplicate
    /// content across the system.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `file_hash` - SHA256 hash bytes to search for
    ///
    /// # Returns
    ///
    /// A vector of `DocumentFile` entries with matching hash,
    /// or a database error if the query fails.
    pub async fn find_files_by_hash(
        conn: &mut AsyncPgConnection,
        file_hash: &[u8],
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::file_hash_sha256.eq(file_hash))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    /// Finds files filtered by their processing status.
    ///
    /// Retrieves files based on their current processing state, enabling
    /// status-based file management and processing pipeline monitoring.
    /// This supports operational visibility and processing workflow
    /// coordination across the file handling system.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `status` - Processing status to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentFile` entries with the specified status, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    pub async fn find_files_by_status(
        conn: &mut AsyncPgConnection,
        status: ProcessingStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::processing_status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    /// Finds files filtered by their virus scan status.
    ///
    /// Retrieves files based on their security scanning state, enabling
    /// security policy enforcement and threat management workflows.
    /// This supports security operations visibility and safe content
    /// access controls within the collaborative environment.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `scan_status` - Virus scan status to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentFile` entries with the specified scan status, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    pub async fn find_files_by_scan_status(
        conn: &mut AsyncPgConnection,
        scan_status: VirusScanStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::virus_scan_status.eq(scan_status))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    /// Finds files with failed processing status for error handling.
    ///
    /// Retrieves files that encountered errors during processing, enabling
    /// error analysis, retry workflows, and operational troubleshooting.
    /// This supports system reliability by providing visibility into
    /// processing failures and facilitating error resolution.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of failed `DocumentFile` entries ordered by failure time
    /// (most recent first), or a database error if the query fails.
    pub async fn find_failed_files(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    /// Finds files exceeding a specified size threshold.
    ///
    /// Retrieves files larger than the specified size limit, enabling
    /// storage management, performance optimization, and policy enforcement
    /// workflows. This supports storage governance and helps identify
    /// files that may require special handling or compression.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `size_threshold` - Minimum file size in bytes to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of large `DocumentFile` entries ordered by size
    /// (largest first), or a database error if the query fails.
    pub async fn find_large_files(
        conn: &mut AsyncPgConnection,
        size_threshold: i64,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentFile>> {
        use schema::document_files::{self, dsl};

        let files = document_files::table
            .filter(dsl::file_size_bytes.gt(size_threshold))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::file_size_bytes.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentFile::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(files)
    }

    /// Gets comprehensive file statistics for a specific document.
    ///
    /// Calculates detailed metrics about file attachments, processing status,
    /// and storage usage within a document. This provides document managers
    /// with insights into attachment patterns, processing health, and
    /// storage optimization opportunities.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document to analyze
    ///
    /// # Returns
    ///
    /// A `DocumentFileStats` structure containing comprehensive document
    /// file metrics, or a database error if the query fails.
    pub async fn get_document_file_stats(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<DocumentFileStats> {
        use schema::document_files::{self, dsl};

        // Total file count
        let total_count: i64 = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count by processing status
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
        let total_size: Option<BigDecimal> = document_files::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first(conn)
            .await
            .map_err(PgError::from)?;

        let total_size = total_size.unwrap_or_else(|| BigDecimal::from(0));

        Ok(DocumentFileStats {
            total_count,
            pending_count,
            processing_count,
            completed_count,
            failed_count,
            total_size,
        })
    }

    /// Gets storage usage statistics for a specific user account.
    ///
    /// Calculates total storage consumption by a user's uploaded files,
    /// providing insights for storage quota management, billing calculations,
    /// and user storage optimization workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the user account to analyze
    ///
    /// # Returns
    ///
    /// The total storage used in bytes as a `BigDecimal`,
    /// or a database error if the query fails.
    pub async fn get_user_storage_usage(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<BigDecimal> {
        use schema::document_files::{self, dsl};

        let usage: Option<BigDecimal> = document_files::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first(conn)
            .await
            .map_err(PgError::from)?;

        Ok(usage.unwrap_or_else(|| BigDecimal::from(0)))
    }

    /// Gets comprehensive processing summary across all files.
    ///
    /// Calculates system-wide file processing metrics and status distribution,
    /// providing operational visibility into processing pipeline health
    /// and performance characteristics across the entire file system.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    ///
    /// # Returns
    ///
    /// A `ProcessingSummary` structure containing system-wide processing
    /// metrics, or a database error if the query fails.
    pub async fn get_processing_summary(
        conn: &mut AsyncPgConnection,
    ) -> PgResult<ProcessingSummary> {
        use schema::document_files::{self, dsl};

        let pending: i64 = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Pending))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let processing: i64 = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Processing))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let completed: i64 = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Completed))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        let failed: i64 = document_files::table
            .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
            .filter(dsl::deleted_at.is_null())
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

    /// Cleans up files marked for automatic deletion.
    ///
    /// Soft deletes files that have been marked for automatic cleanup based
    /// on retention policies or system maintenance schedules. This supports
    /// automated storage management and compliance with data retention
    /// requirements.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    ///
    /// # Returns
    ///
    /// The number of files that were marked for deletion,
    /// or a database error if the operation fails.
    pub async fn cleanup_auto_delete_files(conn: &mut AsyncPgConnection) -> PgResult<usize> {
        use schema::document_files::{self, dsl};

        let affected = diesel::update(document_files::table)
            .filter(dsl::auto_delete_at.le(OffsetDateTime::now_utc()))
            .filter(dsl::deleted_at.is_null())
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    /// Resets failed processing status to allow retry operations.
    ///
    /// Changes failed files back to pending status to enable reprocessing
    /// attempts, supporting error recovery and system resilience workflows.
    /// This helps maintain system reliability by providing retry mechanisms
    /// for transient processing failures.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `file_ids` - Vector of file UUIDs to reset processing status for
    ///
    /// # Returns
    ///
    /// The number of files that were reset to pending status,
    /// or a database error if the operation fails.
    pub async fn reset_failed_processing(
        conn: &mut AsyncPgConnection,
        file_ids: Vec<Uuid>,
    ) -> PgResult<usize> {
        use schema::document_files::{self, dsl};

        let affected = diesel::update(document_files::table)
            .filter(dsl::id.eq_any(file_ids))
            .filter(dsl::processing_status.eq(ProcessingStatus::Failed))
            .set(dsl::processing_status.eq(ProcessingStatus::Pending))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    /// Purges old files beyond retention period.
    ///
    /// Soft deletes files that exceed the specified age threshold,
    /// supporting automated data lifecycle management and storage
    /// optimization policies. This enables compliant data retention
    /// and cost-effective storage management.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `retention_days` - Number of days beyond which files should be purged
    ///
    /// # Returns
    ///
    /// The number of files that were marked for deletion,
    /// or a database error if the operation fails.
    pub async fn purge_old_files(
        conn: &mut AsyncPgConnection,
        retention_days: i32,
    ) -> PgResult<usize> {
        use schema::document_files::{self, dsl};

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(retention_days as i64);

        let affected = diesel::update(document_files::table)
            .filter(dsl::created_at.lt(cutoff_date))
            .filter(dsl::deleted_at.is_null())
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }
}

/// Comprehensive statistics for files within a document.
///
/// Provides insights into file attachment patterns, processing pipeline
/// health, and storage utilization within document workflows. These metrics
/// help document managers understand attachment usage, identify processing
/// bottlenecks, and optimize storage allocation strategies.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentFileStats {
    /// Total number of files attached to the document
    pub total_count: i64,
    /// Files pending processing in the pipeline
    pub pending_count: i64,
    /// Files currently being processed
    pub processing_count: i64,
    /// Successfully processed and available files
    pub completed_count: i64,
    /// Files that failed processing and require attention
    pub failed_count: i64,
    /// Total storage used by all files in bytes
    pub total_size: BigDecimal,
}

impl DocumentFileStats {
    /// Calculates the processing completion rate as a percentage (0-100).
    ///
    /// Returns the percentage of files that have completed processing
    /// successfully, indicating overall processing pipeline health.
    pub fn completion_rate(&self) -> f64 {
        if self.total_count == 0 {
            100.0
        } else {
            (self.completed_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Calculates the processing failure rate as a percentage (0-100).
    ///
    /// Returns the percentage of files that failed processing,
    /// indicating potential system issues requiring attention.
    pub fn failure_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            (self.failed_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Indicates whether there are files still pending processing.
    ///
    /// Returns true if any files are queued or actively being processed,
    /// useful for determining if processing work is ongoing.
    pub fn has_pending_work(&self) -> bool {
        self.pending_count > 0 || self.processing_count > 0
    }

    /// Indicates whether there are failed files requiring attention.
    ///
    /// Returns true if any files have failed processing,
    /// signaling a need for administrative intervention or retry.
    pub fn has_failures(&self) -> bool {
        self.failed_count > 0
    }

    /// Calculates the average file size in bytes.
    ///
    /// Returns the mean file size across all files, useful for
    /// storage planning and optimization analysis.
    pub fn average_file_size(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            let total_size_f64 = self.total_size.to_string().parse::<f64>().unwrap_or(0.0);
            total_size_f64 / self.total_count as f64
        }
    }

    /// Indicates whether the document has any file attachments.
    ///
    /// Returns true if the document contains files,
    /// useful for determining rich media content presence.
    pub fn has_files(&self) -> bool {
        self.total_count > 0
    }

    /// Calculates processing efficiency as a percentage.
    ///
    /// Returns the ratio of successfully processed files to
    /// files that have been attempted, excluding pending files.
    pub fn processing_efficiency(&self) -> f64 {
        let attempted = self.completed_count + self.failed_count;
        if attempted == 0 {
            100.0
        } else {
            (self.completed_count as f64 / attempted as f64) * 100.0
        }
    }
}

/// System-wide file processing pipeline statistics.
///
/// Provides comprehensive visibility into processing queue health,
/// throughput characteristics, and operational status across the
/// entire file processing system. These metrics support capacity
/// planning, performance monitoring, and operational decision making.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessingSummary {
    /// Files queued and waiting for processing
    pub pending: i64,
    /// Files currently being processed by workers
    pub processing: i64,
    /// Files that completed processing successfully
    pub completed: i64,
    /// Files that failed processing and need attention
    pub failed: i64,
}

impl ProcessingSummary {
    /// Calculates the total number of files in the system.
    ///
    /// Returns the sum of all files across all processing states,
    /// providing overall system file volume visibility.
    pub fn total(&self) -> i64 {
        self.pending + self.processing + self.completed + self.failed
    }

    /// Calculates the overall completion rate as a percentage (0-100).
    ///
    /// Returns the percentage of total files that have been
    /// successfully processed, indicating system throughput effectiveness.
    pub fn completion_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            100.0
        } else {
            (self.completed as f64 / total as f64) * 100.0
        }
    }

    /// Indicates whether the processing queue has active work.
    ///
    /// Returns true if files are queued or being processed,
    /// useful for determining system activity and load.
    pub fn is_active(&self) -> bool {
        self.pending > 0 || self.processing > 0
    }

    /// Indicates whether there are processing failures requiring attention.
    ///
    /// Returns true if any files have failed processing,
    /// signaling potential system issues or capacity problems.
    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }

    /// Calculates the failure rate as a percentage (0-100).
    ///
    /// Returns the percentage of files that failed processing,
    /// indicating system reliability and error handling effectiveness.
    pub fn failure_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            (self.failed as f64 / total as f64) * 100.0
        }
    }

    /// Calculates processing throughput efficiency.
    ///
    /// Returns the ratio of completed files to files that have been
    /// attempted (excluding pending), indicating processing success rate.
    pub fn throughput_efficiency(&self) -> f64 {
        let attempted = self.completed + self.failed;
        if attempted == 0 {
            100.0
        } else {
            (self.completed as f64 / attempted as f64) * 100.0
        }
    }

    /// Indicates whether the system is under heavy load.
    ///
    /// Returns true if there are more pending files than completed,
    /// suggesting processing capacity may be insufficient.
    pub fn is_under_heavy_load(&self) -> bool {
        self.pending > self.completed
    }
}
