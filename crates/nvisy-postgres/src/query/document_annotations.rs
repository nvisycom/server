//! Document annotations repository for managing user annotations on documents.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentAnnotation, NewDocumentAnnotation, UpdateDocumentAnnotation};
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for document annotation database operations.
///
/// Handles annotation lifecycle management including creation, updates,
/// filtering by type, and retrieval across files and accounts.
pub trait DocumentAnnotationRepository {
    fn create_annotation(
        &self,
        new_annotation: NewDocumentAnnotation,
    ) -> impl Future<Output = PgResult<DocumentAnnotation>> + Send;

    fn find_annotation_by_id(
        &self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentAnnotation>>> + Send;

    fn find_annotations_by_file(
        &self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    fn find_annotations_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    fn find_annotations_by_type(
        &self,
        file_id: Uuid,
        annotation_type: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    fn update_annotation(
        &self,
        annotation_id: Uuid,
        updates: UpdateDocumentAnnotation,
    ) -> impl Future<Output = PgResult<DocumentAnnotation>> + Send;

    fn delete_annotation(&self, annotation_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    fn count_annotations_by_file(
        &self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    fn count_annotations_by_type(
        &self,
        file_id: Uuid,
        annotation_type: &str,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    fn find_recent_annotations(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    fn check_annotation_ownership(
        &self,
        annotation_id: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

impl DocumentAnnotationRepository for PgClient {
    /// Creates a new document annotation with the provided content and metadata.
    ///
    /// Inserts a new annotation record into the database, enabling users to
    /// mark up, highlight, or comment on specific portions of document files.
    /// This supports collaborative review workflows and document markup capabilities.
    ///
    /// # Arguments
    ///
    /// * `new_annotation` - Data for the new annotation including file, account, content, and type
    ///
    /// # Returns
    ///
    /// The newly created `DocumentAnnotation` with generated ID and timestamps,
    /// or a database error if the operation fails.
    async fn create_annotation(
        &self,
        new_annotation: NewDocumentAnnotation,
    ) -> PgResult<DocumentAnnotation> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations;

        let annotation = diesel::insert_into(document_annotations::table)
            .values(&new_annotation)
            .returning(DocumentAnnotation::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    /// Finds an annotation by its unique identifier.
    ///
    /// Retrieves a specific annotation using its UUID, automatically excluding
    /// soft-deleted annotations. This is the primary method for accessing
    /// individual annotations for viewing, editing, and management operations.
    ///
    /// # Arguments
    ///
    /// * `annotation_id` - UUID of the annotation to retrieve
    ///
    /// # Returns
    ///
    /// The matching `DocumentAnnotation` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
    async fn find_annotation_by_id(
        &self,
        annotation_id: Uuid,
    ) -> PgResult<Option<DocumentAnnotation>> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let annotation = document_annotations::table
            .filter(dsl::id.eq(annotation_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentAnnotation::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    /// Finds all annotations associated with a specific document file.
    ///
    /// Retrieves all active annotations for a file, enabling comprehensive
    /// document markup display and collaborative review workflows. Results
    /// are ordered by creation time for consistent display ordering.
    ///
    /// # Arguments
    ///
    /// * `file_id` - UUID of the document file whose annotations to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of active `DocumentAnnotation` entries for the file, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn find_annotations_by_file(
        &self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentAnnotation>> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let annotations = document_annotations::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentAnnotation::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    /// Finds all annotations created by a specific account.
    ///
    /// Retrieves a user's complete annotation history across all documents,
    /// enabling user activity tracking and personal annotation management.
    /// This supports user dashboards and annotation browsing capabilities.
    ///
    /// # Arguments
    ///
    /// * `account_id` - UUID of the account whose annotations to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentAnnotation` entries created by the account, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn find_annotations_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentAnnotation>> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let annotations = document_annotations::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentAnnotation::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    /// Finds annotations of a specific type for a document file.
    ///
    /// Retrieves annotations filtered by their type (e.g., "note", "highlight"),
    /// enabling type-specific annotation display and filtering in document
    /// review interfaces. This supports specialized annotation workflows
    /// and customized document markup displays.
    ///
    /// # Arguments
    ///
    /// * `file_id` - UUID of the document file whose annotations to retrieve
    /// * `annotation_type` - Type of annotations to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of matching `DocumentAnnotation` entries ordered by creation time
    /// (most recent first), or a database error if the query fails.
    async fn find_annotations_by_type(
        &self,
        file_id: Uuid,
        annotation_type: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentAnnotation>> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let annotations = document_annotations::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::annotation_type.eq(annotation_type))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentAnnotation::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    /// Updates an annotation with new content or metadata.
    ///
    /// Applies partial updates to an existing annotation using the provided
    /// update structure. Only fields set to `Some(value)` will be modified,
    /// while `None` fields remain unchanged. This supports annotation editing
    /// and metadata modifications within document review workflows.
    ///
    /// # Arguments
    ///
    /// * `annotation_id` - UUID of the annotation to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `DocumentAnnotation` with new values and timestamp,
    /// or a database error if the operation fails.
    async fn update_annotation(
        &self,
        annotation_id: Uuid,
        updates: UpdateDocumentAnnotation,
    ) -> PgResult<DocumentAnnotation> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let annotation =
            diesel::update(document_annotations::table.filter(dsl::id.eq(annotation_id)))
                .set(&updates)
                .returning(DocumentAnnotation::as_returning())
                .get_result(&mut conn)
                .await
                .map_err(PgError::from)?;

        Ok(annotation)
    }

    /// Soft deletes an annotation by setting the deletion timestamp.
    ///
    /// Marks an annotation as deleted without permanently removing it from the
    /// database. This preserves annotation history for audit purposes while
    /// preventing the annotation from appearing in normal document views.
    ///
    /// # Arguments
    ///
    /// * `annotation_id` - UUID of the annotation to soft delete
    ///
    /// # Returns
    ///
    /// `()` on successful deletion, or a database error if the operation fails.
    async fn delete_annotation(&self, annotation_id: Uuid) -> PgResult<()> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        diesel::update(document_annotations::table.filter(dsl::id.eq(annotation_id)))
            .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Counts total active annotations for a specific document file.
    ///
    /// Calculates the total number of active annotations associated with a
    /// file, providing annotation activity metrics for document engagement
    /// analysis and user interface displays.
    ///
    /// # Arguments
    ///
    /// * `file_id` - UUID of the document file to count annotations for
    ///
    /// # Returns
    ///
    /// The total count of active annotations for the file,
    /// or a database error if the query fails.
    async fn count_annotations_by_file(&self, file_id: Uuid) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let count = document_annotations::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Counts annotations of a specific type for a document file.
    ///
    /// Calculates the total number of annotations of a given type for a file,
    /// enabling type-specific metrics and annotation distribution analysis.
    ///
    /// # Arguments
    ///
    /// * `file_id` - UUID of the document file to count annotations for
    /// * `annotation_type` - Type of annotations to count
    ///
    /// # Returns
    ///
    /// The total count of matching annotations,
    /// or a database error if the query fails.
    async fn count_annotations_by_type(
        &self,
        file_id: Uuid,
        annotation_type: &str,
    ) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let count = document_annotations::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::annotation_type.eq(annotation_type))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Finds recently created annotations across all documents.
    ///
    /// Retrieves annotations created within the last seven days across the
    /// entire system, providing visibility into recent annotation activity
    /// and enabling activity monitoring and engagement assessment.
    ///
    /// # Arguments
    ///
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of recently created `DocumentAnnotation` entries ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn find_recent_annotations(
        &self,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentAnnotation>> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let seven_days_ago = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().days(7));

        let annotations = document_annotations::table
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentAnnotation::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    /// Checks if an account owns a specific annotation.
    ///
    /// Validates whether a user account is the original creator of an annotation,
    /// supporting annotation editing permissions and access control for
    /// annotation management operations.
    ///
    /// # Arguments
    ///
    /// * `annotation_id` - UUID of the annotation to check ownership for
    /// * `account_id` - UUID of the account claiming ownership
    ///
    /// # Returns
    ///
    /// `true` if the account owns the annotation, `false` otherwise,
    /// or a database error if the query fails.
    async fn check_annotation_ownership(
        &self,
        annotation_id: Uuid,
        account_id: Uuid,
    ) -> PgResult<bool> {
        let mut conn = self.get_connection().await?;

        use schema::document_annotations::{self, dsl};

        let count: i64 = document_annotations::table
            .filter(dsl::id.eq(annotation_id))
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }
}
