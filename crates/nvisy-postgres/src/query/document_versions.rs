//! Document version repository for managing document version table operations.

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentVersion, NewDocumentVersion, UpdateDocumentVersion};
use crate::{PgError, PgResult, schema};

/// Repository for document version table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct DocumentVersionRepository;

impl DocumentVersionRepository {
    /// Creates a new document version repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new document version.
    pub async fn create_document_version(
        conn: &mut AsyncPgConnection,
        new_version: NewDocumentVersion,
    ) -> PgResult<DocumentVersion> {
        use schema::document_versions;

        diesel::insert_into(document_versions::table)
            .values(&new_version)
            .returning(DocumentVersion::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds a document version by its ID.
    pub async fn find_document_version_by_id(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
    ) -> PgResult<Option<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        document_versions::table
            .filter(dsl::id.eq(version_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentVersion::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Lists versions for a document.
    pub async fn list_document_versions(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::version_number.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Lists versions for an account.
    pub async fn list_account_versions(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        document_versions::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Gets the latest version for a document.
    pub async fn get_latest_document_version(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<Option<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::version_number.desc())
            .select(DocumentVersion::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Gets the next version number for a document.
    pub async fn get_next_version_number(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<i32> {
        use schema::document_versions::{self, dsl};

        let max_version = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .select(diesel::dsl::max(dsl::version_number))
            .first::<Option<i32>>(conn)
            .await
            .map_err(PgError::from)?;

        Ok(max_version.unwrap_or(0) + 1)
    }

    /// Finds a specific version by document ID and version number.
    pub async fn find_version_by_number(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        version_number: i32,
    ) -> PgResult<Option<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::version_number.eq(version_number))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentVersion::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Updates a document version.
    pub async fn update_document_version(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
        updates: UpdateDocumentVersion,
    ) -> PgResult<DocumentVersion> {
        use schema::document_versions::{self, dsl};

        diesel::update(document_versions::table.filter(dsl::id.eq(version_id)))
            .set(&updates)
            .returning(DocumentVersion::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Soft deletes a document version.
    pub async fn delete_document_version(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
    ) -> PgResult<()> {
        use schema::document_versions::{self, dsl};

        diesel::update(document_versions::table.filter(dsl::id.eq(version_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    // Query methods

    /// Finds versions created within a date range.
    pub async fn find_versions_by_date_range(
        conn: &mut AsyncPgConnection,
        start_date: OffsetDateTime,
        end_date: OffsetDateTime,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        document_versions::table
            .filter(dsl::created_at.between(start_date, end_date))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds versions by file size range.
    pub async fn find_versions_by_size_range(
        conn: &mut AsyncPgConnection,
        min_size: i64,
        max_size: Option<i64>,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        let mut query = document_versions::table
            .filter(dsl::file_size_bytes.ge(min_size))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::file_size_bytes.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .into_boxed();

        if let Some(max) = max_size {
            query = query.filter(dsl::file_size_bytes.le(max));
        }

        query.load(conn).await.map_err(PgError::from)
    }

    /// Finds large versions above a certain size threshold.
    pub async fn find_large_versions(
        conn: &mut AsyncPgConnection,
        min_size_bytes: i64,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        document_versions::table
            .filter(dsl::file_size_bytes.ge(min_size_bytes))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::file_size_bytes.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds recently created versions.
    pub async fn find_recent_versions(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        use schema::document_versions::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        document_versions::table
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Counts total versions for a document.
    pub async fn count_document_versions(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_versions::{self, dsl};

        document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    // Statistics and analytics

    /// Gets version statistics for a document.
    pub async fn get_document_version_stats(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<DocumentVersionStats> {
        use schema::document_versions::{self, dsl};

        // Total versions
        let total_count: i64 = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        if total_count == 0 {
            return Ok(DocumentVersionStats {
                total_count: 0,
                total_size: 0,
                average_size: 0,
                latest_version_number: 0,
            });
        }

        // Total size
        let total_size: i64 = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first::<Option<BigDecimal>>(conn)
            .await
            .map_err(PgError::from)?
            .map(|bd| bd.to_string().parse().unwrap_or(0))
            .unwrap_or(0);

        // Latest version number
        let latest_version_number: i32 = document_versions::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::max(dsl::version_number))
            .first::<Option<i32>>(conn)
            .await
            .map_err(PgError::from)?
            .unwrap_or(0);

        let average_size = if total_count > 0 {
            total_size / total_count
        } else {
            0
        };

        Ok(DocumentVersionStats {
            total_count,
            total_size,
            average_size,
            latest_version_number,
        })
    }

    /// Gets storage usage for an account's versions.
    pub async fn get_user_version_storage_usage(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_versions::{self, dsl};

        let total_size: i64 = document_versions::table
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

    /// Gets version statistics for a user.
    pub async fn get_user_version_stats(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<UserVersionStats> {
        use schema::document_versions::{self, dsl};

        let now = OffsetDateTime::now_utc();
        let thirty_days_ago = now - time::Duration::days(30);

        // Total versions created by user
        let total_count: i64 = document_versions::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Recent versions
        let recent_count: i64 = document_versions::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::created_at.gt(thirty_days_ago))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Total storage
        let total_storage: i64 = document_versions::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(diesel::dsl::sum(dsl::file_size_bytes))
            .first::<Option<BigDecimal>>(conn)
            .await
            .map_err(PgError::from)?
            .map(|bd| bd.to_string().parse().unwrap_or(0))
            .unwrap_or(0);

        Ok(UserVersionStats {
            total_count,
            recent_count,
            total_storage,
        })
    }

    // Cleanup and maintenance

    /// Cleans up versions marked for auto-deletion.
    pub async fn cleanup_auto_delete_versions(conn: &mut AsyncPgConnection) -> PgResult<i64> {
        use schema::document_versions::{self, dsl};

        diesel::update(
            document_versions::table
                .filter(dsl::auto_delete_at.lt(OffsetDateTime::now_utc()))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }

    /// Hard deletes old soft-deleted versions.
    pub async fn purge_old_versions(
        conn: &mut AsyncPgConnection,
        older_than_days: u32,
    ) -> PgResult<i64> {
        use schema::document_versions::{self, dsl};

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(older_than_days as i64);

        diesel::delete(document_versions::table.filter(dsl::deleted_at.lt(cutoff_date)))
            .execute(conn)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }

    /// Finds orphaned versions (versions whose parent document was deleted).
    pub async fn find_orphaned_versions(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentVersion>> {
        use schema::{document_versions, documents};

        // Find versions where the document doesn't exist or is deleted
        let orphaned_versions = document_versions::table
            .left_join(
                documents::table.on(documents::id
                    .eq(document_versions::document_id)
                    .and(documents::deleted_at.is_null())),
            )
            .filter(documents::id.is_null())
            .filter(document_versions::deleted_at.is_null())
            .order(document_versions::created_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentVersion::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(orphaned_versions)
    }

    /// Cleans up orphaned versions.
    pub async fn cleanup_orphaned_versions(conn: &mut AsyncPgConnection) -> PgResult<i64> {
        use schema::{document_versions, documents};

        // Soft delete versions where the document doesn't exist or is deleted
        // First, find document_ids that don't exist or are deleted
        let orphaned_doc_ids: Vec<Uuid> = document_versions::table
            .left_join(
                documents::table.on(documents::id
                    .eq(document_versions::document_id)
                    .and(documents::deleted_at.is_null())),
            )
            .filter(documents::id.is_null())
            .filter(document_versions::deleted_at.is_null())
            .select(document_versions::document_id)
            .distinct()
            .load(conn)
            .await
            .map_err(PgError::from)?;

        if orphaned_doc_ids.is_empty() {
            return Ok(0);
        }

        // Then update the versions with those document_ids
        diesel::update(
            document_versions::table
                .filter(document_versions::document_id.eq_any(orphaned_doc_ids))
                .filter(document_versions::deleted_at.is_null()),
        )
        .set(document_versions::deleted_at.eq(Some(OffsetDateTime::now_utc())))
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }
}

/// Statistics for document versions.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentVersionStats {
    /// Total number of versions for the document
    pub total_count: i64,
    /// Total file size of all versions in bytes
    pub total_size: i64,
    /// Average version size in bytes
    pub average_size: i64,
    /// Latest version number
    pub latest_version_number: i32,
}

impl DocumentVersionStats {
    /// Returns whether the document has multiple versions.
    pub fn has_multiple_versions(&self) -> bool {
        self.total_count > 1
    }

    /// Returns whether the document has any versions.
    pub fn has_versions(&self) -> bool {
        self.total_count > 0
    }

    /// Returns the total storage usage in a human-readable format.
    pub fn total_size_human(&self) -> String {
        if self.total_size < 1024 {
            format!("{} B", self.total_size)
        } else if self.total_size < 1024 * 1024 {
            format!("{:.1} KB", self.total_size as f64 / 1024.0)
        } else if self.total_size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.total_size as f64 / (1024.0 * 1024.0))
        } else {
            format!(
                "{:.1} GB",
                self.total_size as f64 / (1024.0 * 1024.0 * 1024.0)
            )
        }
    }
}

/// Statistics for versions created by a user.
#[derive(Debug, Clone, PartialEq)]
pub struct UserVersionStats {
    /// Total number of versions created by user
    pub total_count: i64,
    /// Number of versions created in last 30 days
    pub recent_count: i64,
    /// Total storage used by user's versions in bytes
    pub total_storage: i64,
}

impl UserVersionStats {
    /// Returns whether the user is actively creating versions.
    pub fn is_active_versioner(&self) -> bool {
        self.recent_count > 0
    }

    /// Returns the average version size in bytes.
    pub fn average_version_size(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            self.total_storage as f64 / self.total_count as f64
        }
    }

    /// Returns the storage usage in a human-readable format.
    pub fn storage_human(&self) -> String {
        if self.total_storage < 1024 {
            format!("{} B", self.total_storage)
        } else if self.total_storage < 1024 * 1024 {
            format!("{:.1} KB", self.total_storage as f64 / 1024.0)
        } else if self.total_storage < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.total_storage as f64 / (1024.0 * 1024.0))
        } else {
            format!(
                "{:.1} GB",
                self.total_storage as f64 / (1024.0 * 1024.0 * 1024.0)
            )
        }
    }
}
