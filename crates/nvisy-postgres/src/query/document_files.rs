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
