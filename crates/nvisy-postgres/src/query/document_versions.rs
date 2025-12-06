//! Document versions repository for managing version control and history operations.

use std::future::Future;

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentVersion, NewDocumentVersion, UpdateDocumentVersion};
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for document version database operations.
///
/// Handles version control including creation, tracking, sequential numbering,
/// and version history management.
pub trait DocumentVersionRepository {
    fn create_document_version(
        &self,
        new_version: NewDocumentVersion,
    ) -> impl Future<Output = PgResult<DocumentVersion>> + Send;

    fn find_document_version_by_id(
        &self,
        version_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentVersion>>> + Send;

    fn list_document_versions(
        &self,
        document_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentVersion>>> + Send;

    fn list_account_versions(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentVersion>>> + Send;

    fn get_latest_document_version(
        &self,
        document_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentVersion>>> + Send;

    fn get_next_version_number(
        &self,
        document_id: Uuid,
    ) -> impl Future<Output = PgResult<i32>> + Send;

    fn find_version_by_number(
        &self,
        document_id: Uuid,
        version_number: i32,
    ) -> impl Future<Output = PgResult<Option<DocumentVersion>>> + Send;

    fn update_document_version(
        &self,
        version_id: Uuid,
        updates: UpdateDocumentVersion,
    ) -> impl Future<Output = PgResult<DocumentVersion>> + Send;

    fn delete_document_version(
        &self,
        version_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    fn find_versions_by_date_range(
        &self,
        start_date: OffsetDateTime,
        end_date: OffsetDateTime,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentVersion>>> + Send;

    fn find_versions_by_size_range(
        &self,
        min_size: i64,
        max_size: i64,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentVersion>>> + Send;

    fn find_large_versions(
        &self,
        size_threshold: i64,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentVersion>>> + Send;

    fn find_recent_versions(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentVersion>>> + Send;

    fn count_document_versions(
        &self,
        document_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    fn get_user_version_storage_usage(
        &self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<BigDecimal>> + Send;

    fn cleanup_auto_delete_versions(&self) -> impl Future<Output = PgResult<usize>> + Send;

    fn purge_old_versions(
        &self,
        retention_days: i32,
    ) -> impl Future<Output = PgResult<usize>> + Send;

    fn find_orphaned_versions(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentVersion>>> + Send;

    fn cleanup_orphaned_versions(&self) -> impl Future<Output = PgResult<usize>> + Send;
}

impl DocumentVersionRepository for PgClient {
    async fn create_document_version(
        &self,
        new_version: NewDocumentVersion,
    ) -> PgResult<DocumentVersion> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions;

        let version = diesel::insert_into(document_versions::table)
            .values(&new_version)
            .returning(DocumentVersion::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(version)
    }

    /// Finds a document version by its unique identifier.
    ///
    /// Retrieves a specific version record using its UUID, automatically
    /// excluding soft-deleted versions. This is the primary method for
    /// accessing individual versions for content retrieval, comparison
    /// operations, and version-specific metadata queries.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `version_id` - UUID of the version to retrieve
    ///
    /// # Returns
    ///
    /// The matching `DocumentVersion` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
    async fn find_document_version_by_id(
        &self,
        version_id: Uuid,
    ) -> PgResult<Option<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let version = document_versions::table
            .filter(dsl::id.eq(version_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentVersion::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(version)
    }

    /// Lists versions for a specific document with chronological ordering.
    ///
    /// Retrieves a paginated list of versions for a document, ordered by
    /// version number in descending order (newest first). This supports
    /// version history interfaces and enables users to browse document
    /// evolution over time with full version metadata.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document whose versions to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentVersion` entries for the document, ordered by
    /// version number (newest first), or a database error if the query fails.
    async fn list_document_versions(
        &self,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let versions = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::version_number.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(versions)
    }

    /// Lists versions created by a specific account across all documents.
    ///
    /// Retrieves a paginated list of versions authored by a user account,
    /// ordered by creation time (most recent first). This supports user
    /// activity tracking, contribution analysis, and personal version
    /// management workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account whose versions to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentVersion` entries created by the account, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn list_account_versions(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let versions = document_versions::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(versions)
    }

    /// Gets the latest version for a document.
    ///
    /// Retrieves the most recent version of a document based on version
    /// numbering, providing access to the current state of document content.
    /// This supports current content retrieval and latest version comparison
    /// operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document whose latest version to retrieve
    ///
    /// # Returns
    ///
    /// The latest `DocumentVersion` for the document if any versions exist,
    /// `None` if no versions found, or a database error if the query fails.
    async fn get_latest_document_version(
        &self,
        document_id: Uuid,
    ) -> PgResult<Option<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let version = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::version_number.desc())
            .select(DocumentVersion::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(version)
    }

    /// Gets the next version number for a document.
    ///
    /// Calculates the appropriate version number for a new version by
    /// finding the highest existing version number and incrementing it.
    /// This ensures sequential version numbering and prevents version
    /// number conflicts during concurrent editing scenarios.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document to get next version number for
    ///
    /// # Returns
    ///
    /// The next sequential version number for the document,
    /// or a database error if the query fails.
    async fn get_next_version_number(&self, document_id: Uuid) -> PgResult<i32> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let max_version: Option<i32> = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::max(dsl::version_number))
            .first(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(max_version.unwrap_or(0) + 1)
    }

    /// Finds a specific version by document and version number.
    ///
    /// Retrieves a version using the combination of document ID and version
    /// number, enabling direct access to specific versions in the document
    /// history. This supports version comparison, rollback operations, and
    /// historical content retrieval workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document containing the version
    /// * `version_number` - Specific version number to retrieve
    ///
    /// # Returns
    ///
    /// The matching `DocumentVersion` if found, `None` if not found,
    /// or a database error if the query fails.
    async fn find_version_by_number(
        &self,
        document_id: Uuid,
        version_number: i32,
    ) -> PgResult<Option<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let version = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::version_number.eq(version_number))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentVersion::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(version)
    }

    /// Updates a document version with new metadata and information.
    ///
    /// Applies partial updates to an existing version record using the provided
    /// update structure. Only fields set to `Some(value)` will be modified,
    /// while `None` fields remain unchanged. This supports version metadata
    /// enrichment and administrative version management operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `version_id` - UUID of the version to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `DocumentVersion` with new values,
    /// or a database error if the operation fails.
    async fn update_document_version(
        &self,
        version_id: Uuid,
        updates: UpdateDocumentVersion,
    ) -> PgResult<DocumentVersion> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let version = diesel::update(document_versions::table.filter(dsl::id.eq(version_id)))
            .set(&updates)
            .returning(DocumentVersion::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(version)
    }

    /// Soft deletes a document version by setting the deletion timestamp.
    ///
    /// Marks a version as deleted without permanently removing it from the
    /// database. This preserves version history for audit purposes and
    /// compliance requirements while preventing the version from appearing
    /// in normal version listings and comparison operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `version_id` - UUID of the version to soft delete
    ///
    /// # Returns
    ///
    /// `()` on successful deletion, or a database error if the operation fails.
    ///
    /// # Important Considerations
    ///
    /// Deleting versions may impact document history integrity and rollback
    /// capabilities. Consider the implications for audit trails and compliance
    /// requirements before performing this operation.
    async fn delete_document_version(&self, version_id: Uuid) -> PgResult<()> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        diesel::update(document_versions::table.filter(dsl::id.eq(version_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Finds versions created within a specific date range.
    ///
    /// Retrieves versions based on their creation timestamp, enabling
    /// temporal analysis of document editing activity and content evolution
    /// patterns. This supports activity reporting and historical analysis
    /// workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `start_date` - Beginning of the date range (inclusive)
    /// * `end_date` - End of the date range (inclusive)
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentVersion` entries created within the date range,
    /// ordered by creation time (most recent first), or a database error if the query fails.
    async fn find_versions_by_date_range(
        &self,
        start_date: OffsetDateTime,
        end_date: OffsetDateTime,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let versions = document_versions::table
            .filter(dsl::created_at.between(start_date, end_date))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(versions)
    }

    /// Finds versions within a specific content size range.
    ///
    /// Retrieves versions based on their content size in bytes, enabling
    /// content size analysis, storage optimization, and content management
    /// workflows. This supports storage planning and content governance
    /// operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `min_size` - Minimum content size in bytes
    /// * `max_size` - Maximum content size in bytes
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentVersion` entries within the size range,
    /// ordered by content size (largest first), or a database error if the query fails.
    async fn find_versions_by_size_range(
        &self,
        min_size: i64,
        max_size: i64,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let versions = document_versions::table
            .filter(dsl::file_size_bytes.between(min_size, max_size))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::file_size_bytes.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(versions)
    }

    /// Finds versions exceeding a specified content size threshold.
    ///
    /// Retrieves versions larger than the specified size limit, enabling
    /// large content identification, storage optimization, and performance
    /// management workflows. This supports content governance and helps
    /// identify versions that may require special handling.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `size_threshold` - Minimum content size threshold in bytes
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of large `DocumentVersion` entries ordered by content size
    /// (largest first), or a database error if the query fails.
    async fn find_large_versions(
        &self,
        size_threshold: i64,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let versions = document_versions::table
            .filter(dsl::file_size_bytes.gt(size_threshold))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::file_size_bytes.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(versions)
    }

    /// Finds recently created versions across all documents.
    ///
    /// Retrieves versions created within the last seven days across the
    /// entire system, providing visibility into recent editing activity
    /// and content evolution patterns. This supports activity monitoring
    /// and content discovery workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of recently created `DocumentVersion` entries ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn find_recent_versions(&self, pagination: Pagination) -> PgResult<Vec<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        let versions = document_versions::table
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(versions)
    }

    /// Counts total active versions for a specific document.
    ///
    /// Calculates the total number of active versions associated with a
    /// document, providing version count metrics for document management
    /// and version control analytics.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document to count versions for
    ///
    /// # Returns
    ///
    /// The total count of active versions for the document,
    /// or a database error if the query fails.
    async fn count_document_versions(&self, document_id: Uuid) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let count = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Gets storage usage statistics for a specific user account.
    ///
    /// Calculates total storage consumption by a user's created versions,
    /// providing insights for storage quota management and user activity
    /// analysis across version control workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the user account to analyze
    ///
    /// # Returns
    ///
    /// The total storage used by the user's versions in bytes as a `BigDecimal`,
    /// or a database error if the query fails.
    async fn get_user_version_storage_usage(&self, account_id: Uuid) -> PgResult<BigDecimal> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let usage: Option<BigDecimal> = document_versions::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(usage.unwrap_or_else(|| BigDecimal::from(0)))
    }

    /// Cleans up versions marked for automatic deletion.
    ///
    /// Soft deletes versions that have been marked for automatic cleanup based
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
    /// The number of versions that were marked for deletion,
    /// or a database error if the operation fails.
    async fn cleanup_auto_delete_versions(&self) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let affected = diesel::update(document_versions::table)
            .filter(dsl::auto_delete_at.le(OffsetDateTime::now_utc()))
            .filter(dsl::deleted_at.is_null())
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    /// Purges old versions beyond retention period.
    ///
    /// Soft deletes versions that exceed the specified age threshold,
    /// supporting automated data lifecycle management and storage
    /// optimization policies. This enables compliant data retention
    /// and cost-effective storage management.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `retention_days` - Number of days beyond which versions should be purged
    ///
    /// # Returns
    ///
    /// The number of versions that were marked for deletion,
    /// or a database error if the operation fails.
    async fn purge_old_versions(&self, retention_days: i32) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::{self, dsl};

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(retention_days as i64);

        let affected = diesel::update(document_versions::table)
            .filter(dsl::created_at.lt(cutoff_date))
            .filter(dsl::deleted_at.is_null())
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    /// Finds orphaned versions without associated documents.
    ///
    /// Retrieves versions that reference documents that no longer exist,
    /// enabling data integrity maintenance and cleanup operations. This
    /// supports database consistency and helps identify data relationship
    /// issues that may require administrative attention.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of orphaned `DocumentVersion` entries,
    /// or a database error if the query fails.
    async fn find_orphaned_versions(
        &self,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::dsl as version_dsl;
        use schema::documents::dsl as doc_dsl;
        use schema::{document_versions, documents};

        let versions = document_versions::table
            .left_join(documents::table.on(doc_dsl::id.eq(version_dsl::document_id)))
            .filter(doc_dsl::id.is_null())
            .filter(version_dsl::deleted_at.is_null())
            .order(version_dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(versions)
    }

    /// Cleans up orphaned versions without associated documents.
    ///
    /// Soft deletes versions that reference non-existent documents,
    /// maintaining database consistency and preventing data integrity
    /// issues. This supports automated database maintenance and ensures
    /// referential integrity within the version control system.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    ///
    /// # Returns
    ///
    /// The number of orphaned versions that were marked for deletion,
    /// or a database error if the operation fails.
    async fn cleanup_orphaned_versions(&self) -> PgResult<usize> {
        let mut conn = self.get_connection().await?;

        use schema::document_versions::dsl as version_dsl;
        use schema::documents::dsl as doc_dsl;
        use schema::{document_versions, documents};

        let affected = diesel::update(document_versions::table)
            .filter(
                version_dsl::document_id.ne_all(
                    documents::table
                        .filter(doc_dsl::deleted_at.is_null())
                        .select(doc_dsl::id),
                ),
            )
            .filter(version_dsl::deleted_at.is_null())
            .set(version_dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }
}
