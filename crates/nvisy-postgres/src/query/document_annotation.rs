//! Document annotations repository for managing user annotations on documents.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentAnnotation, NewDocumentAnnotation, UpdateDocumentAnnotation};
use crate::types::AnnotationType;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for document annotation database operations.
///
/// Handles annotation lifecycle management including creation, updates,
/// filtering by type, and retrieval across files and accounts.
pub trait DocumentAnnotationRepository {
    /// Creates a new document annotation.
    fn create_annotation(
        &mut self,
        new_annotation: NewDocumentAnnotation,
    ) -> impl Future<Output = PgResult<DocumentAnnotation>> + Send;

    /// Finds an annotation by its unique identifier.
    fn find_annotation_by_id(
        &mut self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentAnnotation>>> + Send;

    /// Finds all annotations for a specific document file.
    fn find_annotations_by_file(
        &mut self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    /// Finds all annotations created by a specific account.
    fn find_annotations_by_account(
        &mut self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    /// Finds annotations of a specific type for a document file.
    fn find_annotations_by_type(
        &mut self,
        file_id: Uuid,
        annotation_type: AnnotationType,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    /// Updates an annotation with new content or metadata.
    fn update_annotation(
        &mut self,
        annotation_id: Uuid,
        updates: UpdateDocumentAnnotation,
    ) -> impl Future<Output = PgResult<DocumentAnnotation>> + Send;

    /// Soft deletes an annotation by setting the deletion timestamp.
    fn delete_annotation(
        &mut self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Finds annotations created within the last 7 days.
    fn find_recent_annotations(
        &mut self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    /// Checks if an account owns a specific annotation.
    fn check_annotation_ownership(
        &mut self,
        annotation_id: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

impl DocumentAnnotationRepository for PgConnection {
    async fn create_annotation(
        &mut self,
        new_annotation: NewDocumentAnnotation,
    ) -> PgResult<DocumentAnnotation> {
        use schema::document_annotations;

        let annotation = diesel::insert_into(document_annotations::table)
            .values(&new_annotation)
            .returning(DocumentAnnotation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn find_annotation_by_id(
        &mut self,
        annotation_id: Uuid,
    ) -> PgResult<Option<DocumentAnnotation>> {
        use schema::document_annotations::{self, dsl};

        let annotation = document_annotations::table
            .filter(dsl::id.eq(annotation_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentAnnotation::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn find_annotations_by_file(
        &mut self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentAnnotation>> {
        use schema::document_annotations::{self, dsl};

        let annotations = document_annotations::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentAnnotation::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    async fn find_annotations_by_account(
        &mut self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentAnnotation>> {
        use schema::document_annotations::{self, dsl};

        let annotations = document_annotations::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentAnnotation::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    async fn find_annotations_by_type(
        &mut self,
        file_id: Uuid,
        annotation_type: AnnotationType,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentAnnotation>> {
        use schema::document_annotations::{self, dsl};

        let annotations = document_annotations::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::annotation_type.eq(annotation_type))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentAnnotation::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    async fn update_annotation(
        &mut self,
        annotation_id: Uuid,
        updates: UpdateDocumentAnnotation,
    ) -> PgResult<DocumentAnnotation> {
        use schema::document_annotations::{self, dsl};

        let annotation =
            diesel::update(document_annotations::table.filter(dsl::id.eq(annotation_id)))
                .set(&updates)
                .returning(DocumentAnnotation::as_returning())
                .get_result(self)
                .await
                .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn delete_annotation(&mut self, annotation_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::document_annotations::{self, dsl};

        diesel::update(document_annotations::table.filter(dsl::id.eq(annotation_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn find_recent_annotations(
        &mut self,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentAnnotation>> {
        use schema::document_annotations::{self, dsl};

        let seven_days_ago = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().days(7));

        let annotations = document_annotations::table
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentAnnotation::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    async fn check_annotation_ownership(
        &mut self,
        annotation_id: Uuid,
        account_id: Uuid,
    ) -> PgResult<bool> {
        use schema::document_annotations::{self, dsl};

        let count: i64 = document_annotations::table
            .filter(dsl::id.eq(annotation_id))
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }
}
