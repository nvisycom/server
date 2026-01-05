//! Document annotations repository for managing user annotations on documents.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{DocumentAnnotation, NewDocumentAnnotation, UpdateDocumentAnnotation};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for document annotation database operations.
///
/// Handles annotation lifecycle management including creation, updates,
/// filtering by type, and retrieval across files and accounts.
pub trait DocumentAnnotationRepository {
    /// Creates a new document annotation.
    fn create_document_annotation(
        &mut self,
        new_annotation: NewDocumentAnnotation,
    ) -> impl Future<Output = PgResult<DocumentAnnotation>> + Send;

    /// Finds a document annotation by its unique identifier.
    fn find_document_annotation_by_id(
        &mut self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentAnnotation>>> + Send;

    /// Lists document annotations for a file with offset pagination.
    fn offset_list_file_document_annotations(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    /// Lists document annotations for a file with cursor pagination.
    fn cursor_list_file_document_annotations(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<DocumentAnnotation>>> + Send;

    /// Lists document annotations created by an account with offset pagination.
    fn offset_list_account_document_annotations(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentAnnotation>>> + Send;

    /// Lists document annotations created by an account with cursor pagination.
    fn cursor_list_account_document_annotations(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<DocumentAnnotation>>> + Send;

    /// Updates a document annotation.
    fn update_document_annotation(
        &mut self,
        annotation_id: Uuid,
        updates: UpdateDocumentAnnotation,
    ) -> impl Future<Output = PgResult<DocumentAnnotation>> + Send;

    /// Soft deletes a document annotation.
    fn delete_document_annotation(
        &mut self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;
}

impl DocumentAnnotationRepository for PgConnection {
    async fn create_document_annotation(
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

    async fn find_document_annotation_by_id(
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

    async fn offset_list_file_document_annotations(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
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

    async fn cursor_list_file_document_annotations(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<DocumentAnnotation>> {
        use diesel::dsl::count_star;
        use schema::document_annotations::{self, dsl};

        let base_filter = dsl::document_file_id
            .eq(file_id)
            .and(dsl::deleted_at.is_null());

        let total = if pagination.include_count {
            Some(
                document_annotations::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            document_annotations::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(DocumentAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            document_annotations::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(DocumentAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |a| {
            (a.created_at.into(), a.id)
        }))
    }

    async fn offset_list_account_document_annotations(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
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

    async fn cursor_list_account_document_annotations(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<DocumentAnnotation>> {
        use diesel::dsl::count_star;
        use schema::document_annotations::{self, dsl};

        let base_filter = dsl::account_id
            .eq(account_id)
            .and(dsl::deleted_at.is_null());

        let total = if pagination.include_count {
            Some(
                document_annotations::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            document_annotations::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(DocumentAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            document_annotations::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(DocumentAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |a| {
            (a.created_at.into(), a.id)
        }))
    }

    async fn update_document_annotation(
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

    async fn delete_document_annotation(&mut self, annotation_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::document_annotations::{self, dsl};

        diesel::update(document_annotations::table.filter(dsl::id.eq(annotation_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }
}
