//! Workspace file annotations repository for managing user annotations on files.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceFileAnnotation, UpdateWorkspaceFileAnnotation, WorkspaceFileAnnotation,
};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace file annotation database operations.
///
/// Handles annotation lifecycle management including creation, updates,
/// filtering by type, and retrieval across files and accounts.
pub trait WorkspaceFileAnnotationRepository {
    /// Creates a new workspace file annotation.
    fn create_workspace_file_annotation(
        &mut self,
        new_annotation: NewWorkspaceFileAnnotation,
    ) -> impl Future<Output = PgResult<WorkspaceFileAnnotation>> + Send;

    /// Finds a workspace file annotation by its unique identifier.
    fn find_workspace_file_annotation_by_id(
        &mut self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceFileAnnotation>>> + Send;

    /// Lists workspace file annotations for a file with offset pagination.
    fn offset_list_workspace_file_annotations(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceFileAnnotation>>> + Send;

    /// Lists workspace file annotations for a file with cursor pagination.
    fn cursor_list_workspace_file_annotations(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceFileAnnotation>>> + Send;

    /// Lists workspace file annotations created by an account with offset pagination.
    fn offset_list_account_workspace_file_annotations(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceFileAnnotation>>> + Send;

    /// Lists workspace file annotations created by an account with cursor pagination.
    fn cursor_list_account_workspace_file_annotations(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceFileAnnotation>>> + Send;

    /// Updates a workspace file annotation.
    fn update_workspace_file_annotation(
        &mut self,
        annotation_id: Uuid,
        updates: UpdateWorkspaceFileAnnotation,
    ) -> impl Future<Output = PgResult<WorkspaceFileAnnotation>> + Send;

    /// Soft deletes a workspace file annotation.
    fn delete_workspace_file_annotation(
        &mut self,
        annotation_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;
}

impl WorkspaceFileAnnotationRepository for PgConnection {
    async fn create_workspace_file_annotation(
        &mut self,
        new_annotation: NewWorkspaceFileAnnotation,
    ) -> PgResult<WorkspaceFileAnnotation> {
        use schema::file_annotations;

        let annotation = diesel::insert_into(file_annotations::table)
            .values(&new_annotation)
            .returning(WorkspaceFileAnnotation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn find_workspace_file_annotation_by_id(
        &mut self,
        annotation_id: Uuid,
    ) -> PgResult<Option<WorkspaceFileAnnotation>> {
        use schema::file_annotations::{self, dsl};

        let annotation = file_annotations::table
            .filter(dsl::id.eq(annotation_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceFileAnnotation::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn offset_list_workspace_file_annotations(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceFileAnnotation>> {
        use schema::file_annotations::{self, dsl};

        let annotations = file_annotations::table
            .filter(dsl::file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceFileAnnotation::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    async fn cursor_list_workspace_file_annotations(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WorkspaceFileAnnotation>> {
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
                .select(WorkspaceFileAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            file_annotations::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(WorkspaceFileAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |a| {
            (a.created_at.into(), a.id)
        }))
    }

    async fn offset_list_account_workspace_file_annotations(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceFileAnnotation>> {
        use schema::file_annotations::{self, dsl};

        let annotations = file_annotations::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceFileAnnotation::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotations)
    }

    async fn cursor_list_account_workspace_file_annotations(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WorkspaceFileAnnotation>> {
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
                .select(WorkspaceFileAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            file_annotations::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(WorkspaceFileAnnotation::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |a| {
            (a.created_at.into(), a.id)
        }))
    }

    async fn update_workspace_file_annotation(
        &mut self,
        annotation_id: Uuid,
        updates: UpdateWorkspaceFileAnnotation,
    ) -> PgResult<WorkspaceFileAnnotation> {
        use schema::file_annotations::{self, dsl};

        let annotation = diesel::update(file_annotations::table.filter(dsl::id.eq(annotation_id)))
            .set(&updates)
            .returning(WorkspaceFileAnnotation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(annotation)
    }

    async fn delete_workspace_file_annotation(&mut self, annotation_id: Uuid) -> PgResult<()> {
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
