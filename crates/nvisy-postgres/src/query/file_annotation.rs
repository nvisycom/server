//! File annotations repository for managing user annotations on files.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{FileAnnotation, NewFileAnnotation, UpdateFileAnnotation};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for file annotation database operations.
///
/// Handles annotation lifecycle management including creation, updates,
/// filtering by type, and retrieval across files and accounts.
pub trait FileAnnotationRepository {
    /// Creates a new file annotation.
    fn create_file_annotation(
        &mut self,
        new_annotation: NewFileAnnotation,
    ) -> impl Future<Output = PgResult<FileAnnotation>> + Send;

    /// Finds a file annotation by its unique identifier.
    fn find_file_annotation_by_id(
        &mut self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<FileAnnotation>>> + Send;

    /// Lists file annotations for a file with offset pagination.
    fn offset_list_file_annotations(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<FileAnnotation>>> + Send;

    /// Lists file annotations for a file with cursor pagination.
    fn cursor_list_file_annotations(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<FileAnnotation>>> + Send;

    /// Lists file annotations created by an account with offset pagination.
    fn offset_list_account_file_annotations(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<FileAnnotation>>> + Send;

    /// Lists file annotations created by an account with cursor pagination.
    fn cursor_list_account_file_annotations(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<FileAnnotation>>> + Send;

    /// Updates a file annotation.
    fn update_file_annotation(
        &mut self,
        annotation_id: Uuid,
        updates: UpdateFileAnnotation,
    ) -> impl Future<Output = PgResult<FileAnnotation>> + Send;

    /// Soft deletes a file annotation.
    fn delete_file_annotation(
        &mut self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;
}

impl FileAnnotationRepository for PgConnection {
    async fn create_file_annotation(
        &mut self,
        new_annotation: NewFileAnnotation,
    ) -> PgResult<FileAnnotation> {
        use schema::file_annotations;

        let annotation = diesel::insert_into(file_annotations::table)
            .values(&new_annotation)
            .returning(FileAnnotation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn find_file_annotation_by_id(
        &mut self,
        annotation_id: Uuid,
    ) -> PgResult<Option<FileAnnotation>> {
        use schema::file_annotations::{self, dsl};

        let annotation = file_annotations::table
            .filter(dsl::id.eq(annotation_id))
            .filter(dsl::deleted_at.is_null())
            .select(FileAnnotation::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn offset_list_file_annotations(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<FileAnnotation>> {
        use schema::file_annotations::{self, dsl};

        let annotations = file_annotations::table
            .filter(dsl::file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(FileAnnotation::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    async fn cursor_list_file_annotations(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<FileAnnotation>> {
        use diesel::dsl::count_star;
        use schema::file_annotations::{self, dsl};

        let base_filter = dsl::file_id.eq(file_id).and(dsl::deleted_at.is_null());

        let total = if pagination.include_count {
            Some(
                file_annotations::table
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
            file_annotations::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(FileAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            file_annotations::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(FileAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |a| {
            (a.created_at.into(), a.id)
        }))
    }

    async fn offset_list_account_file_annotations(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<FileAnnotation>> {
        use schema::file_annotations::{self, dsl};

        let annotations = file_annotations::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(FileAnnotation::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    async fn cursor_list_account_file_annotations(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<FileAnnotation>> {
        use diesel::dsl::count_star;
        use schema::file_annotations::{self, dsl};

        let base_filter = dsl::account_id
            .eq(account_id)
            .and(dsl::deleted_at.is_null());

        let total = if pagination.include_count {
            Some(
                file_annotations::table
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
            file_annotations::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(FileAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            file_annotations::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(FileAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |a| {
            (a.created_at.into(), a.id)
        }))
    }

    async fn update_file_annotation(
        &mut self,
        annotation_id: Uuid,
        updates: UpdateFileAnnotation,
    ) -> PgResult<FileAnnotation> {
        use schema::file_annotations::{self, dsl};

        let annotation = diesel::update(file_annotations::table.filter(dsl::id.eq(annotation_id)))
            .set(&updates)
            .returning(FileAnnotation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn delete_file_annotation(&mut self, annotation_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::file_annotations::{self, dsl};

        diesel::update(file_annotations::table.filter(dsl::id.eq(annotation_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }
}
