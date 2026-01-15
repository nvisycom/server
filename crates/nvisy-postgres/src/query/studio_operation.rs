//! Studio operation repository for managing document operations (diffs).

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewStudioOperation, StudioOperation, UpdateStudioOperation};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for studio operation database operations.
///
/// Handles document operation tracking including CRUD operations, apply/revert
/// state management, and querying by tool call or file.
pub trait StudioOperationRepository {
    /// Creates a new studio operation.
    fn create_studio_operation(
        &mut self,
        operation: NewStudioOperation,
    ) -> impl Future<Output = PgResult<StudioOperation>> + Send;

    /// Creates multiple studio operations in a batch.
    fn create_studio_operations(
        &mut self,
        operations: Vec<NewStudioOperation>,
    ) -> impl Future<Output = PgResult<Vec<StudioOperation>>> + Send;

    /// Finds a studio operation by its unique identifier.
    fn find_studio_operation_by_id(
        &mut self,
        operation_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<StudioOperation>>> + Send;

    /// Updates an existing studio operation.
    fn update_studio_operation(
        &mut self,
        operation_id: Uuid,
        changes: UpdateStudioOperation,
    ) -> impl Future<Output = PgResult<StudioOperation>> + Send;

    /// Deletes a studio operation.
    fn delete_studio_operation(
        &mut self,
        operation_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists operations for a tool call.
    fn list_tool_call_operations(
        &mut self,
        tool_call_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<StudioOperation>>> + Send;

    /// Lists operations for a file with offset pagination.
    fn offset_list_file_operations(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<StudioOperation>>> + Send;

    /// Lists operations for a file with cursor pagination.
    fn cursor_list_file_operations(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<StudioOperation>>> + Send;

    /// Lists pending (unapplied) operations for a file.
    fn list_pending_file_operations(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<StudioOperation>>> + Send;

    /// Marks an operation as applied.
    fn apply_studio_operation(
        &mut self,
        operation_id: Uuid,
    ) -> impl Future<Output = PgResult<StudioOperation>> + Send;

    /// Marks multiple operations as applied.
    fn apply_studio_operations(
        &mut self,
        operation_ids: Vec<Uuid>,
    ) -> impl Future<Output = PgResult<Vec<StudioOperation>>> + Send;

    /// Marks an operation as reverted.
    fn revert_studio_operation(
        &mut self,
        operation_id: Uuid,
    ) -> impl Future<Output = PgResult<StudioOperation>> + Send;

    /// Counts operations by status for a file.
    fn count_file_operations(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<FileOperationCounts>> + Send;
}

/// Counts of operations by status for a file.
#[derive(Debug, Clone, Default)]
pub struct FileOperationCounts {
    /// Total number of operations.
    pub total: i64,
    /// Number of applied operations.
    pub applied: i64,
    /// Number of pending (unapplied) operations.
    pub pending: i64,
    /// Number of reverted operations.
    pub reverted: i64,
}

impl StudioOperationRepository for PgConnection {
    async fn create_studio_operation(
        &mut self,
        operation: NewStudioOperation,
    ) -> PgResult<StudioOperation> {
        use schema::studio_operations;

        let operation = diesel::insert_into(studio_operations::table)
            .values(&operation)
            .returning(StudioOperation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(operation)
    }

    async fn create_studio_operations(
        &mut self,
        operations: Vec<NewStudioOperation>,
    ) -> PgResult<Vec<StudioOperation>> {
        use schema::studio_operations;

        let operations = diesel::insert_into(studio_operations::table)
            .values(&operations)
            .returning(StudioOperation::as_returning())
            .get_results(self)
            .await
            .map_err(PgError::from)?;

        Ok(operations)
    }

    async fn find_studio_operation_by_id(
        &mut self,
        operation_id: Uuid,
    ) -> PgResult<Option<StudioOperation>> {
        use schema::studio_operations::dsl::*;

        let operation = studio_operations
            .filter(id.eq(operation_id))
            .select(StudioOperation::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(operation)
    }

    async fn update_studio_operation(
        &mut self,
        operation_id: Uuid,
        changes: UpdateStudioOperation,
    ) -> PgResult<StudioOperation> {
        use schema::studio_operations::dsl::*;

        let operation = diesel::update(studio_operations)
            .filter(id.eq(operation_id))
            .set(&changes)
            .returning(StudioOperation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(operation)
    }

    async fn delete_studio_operation(&mut self, operation_id: Uuid) -> PgResult<()> {
        use schema::studio_operations::dsl::*;

        diesel::delete(studio_operations)
            .filter(id.eq(operation_id))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn list_tool_call_operations(&mut self, tc_id: Uuid) -> PgResult<Vec<StudioOperation>> {
        use schema::studio_operations::{self, dsl};

        let operations = studio_operations::table
            .filter(dsl::tool_call_id.eq(tc_id))
            .select(StudioOperation::as_select())
            .order(dsl::created_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(operations)
    }

    async fn offset_list_file_operations(
        &mut self,
        f_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<StudioOperation>> {
        use schema::studio_operations::{self, dsl};

        let operations = studio_operations::table
            .filter(dsl::file_id.eq(f_id))
            .select(StudioOperation::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(operations)
    }

    async fn cursor_list_file_operations(
        &mut self,
        f_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<StudioOperation>> {
        use schema::studio_operations::{self, dsl};

        let total = if pagination.include_count {
            Some(
                studio_operations::table
                    .filter(dsl::file_id.eq(f_id))
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let limit = pagination.limit + 1;

        let items: Vec<StudioOperation> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            studio_operations::table
                .filter(dsl::file_id.eq(f_id))
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(StudioOperation::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            studio_operations::table
                .filter(dsl::file_id.eq(f_id))
                .select(StudioOperation::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |op: &StudioOperation| (op.created_at.into(), op.id),
        ))
    }

    async fn list_pending_file_operations(&mut self, f_id: Uuid) -> PgResult<Vec<StudioOperation>> {
        use schema::studio_operations::{self, dsl};

        let operations = studio_operations::table
            .filter(dsl::file_id.eq(f_id))
            .filter(dsl::applied.eq(false))
            .select(StudioOperation::as_select())
            .order(dsl::created_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(operations)
    }

    async fn apply_studio_operation(&mut self, operation_id: Uuid) -> PgResult<StudioOperation> {
        let changes = UpdateStudioOperation {
            applied: Some(true),
            applied_at: Some(Some(jiff_diesel::Timestamp::from(jiff::Timestamp::now()))),
            ..Default::default()
        };

        self.update_studio_operation(operation_id, changes).await
    }

    async fn apply_studio_operations(
        &mut self,
        operation_ids: Vec<Uuid>,
    ) -> PgResult<Vec<StudioOperation>> {
        use schema::studio_operations::dsl::*;

        let now = jiff_diesel::Timestamp::from(jiff::Timestamp::now());

        let operations = diesel::update(studio_operations)
            .filter(id.eq_any(&operation_ids))
            .set((applied.eq(true), applied_at.eq(Some(now))))
            .returning(StudioOperation::as_returning())
            .get_results(self)
            .await
            .map_err(PgError::from)?;

        Ok(operations)
    }

    async fn revert_studio_operation(&mut self, operation_id: Uuid) -> PgResult<StudioOperation> {
        let changes = UpdateStudioOperation {
            reverted: Some(true),
            ..Default::default()
        };

        self.update_studio_operation(operation_id, changes).await
    }

    async fn count_file_operations(&mut self, f_id: Uuid) -> PgResult<FileOperationCounts> {
        use diesel::dsl::count_star;
        use schema::studio_operations::{self, dsl};

        let total = studio_operations::table
            .filter(dsl::file_id.eq(f_id))
            .select(count_star())
            .get_result::<i64>(self)
            .await
            .map_err(PgError::from)?;

        let applied_count = studio_operations::table
            .filter(dsl::file_id.eq(f_id))
            .filter(dsl::applied.eq(true))
            .select(count_star())
            .get_result::<i64>(self)
            .await
            .map_err(PgError::from)?;

        let reverted_count = studio_operations::table
            .filter(dsl::file_id.eq(f_id))
            .filter(dsl::reverted.eq(true))
            .select(count_star())
            .get_result::<i64>(self)
            .await
            .map_err(PgError::from)?;

        Ok(FileOperationCounts {
            total,
            applied: applied_count,
            pending: total - applied_count,
            reverted: reverted_count,
        })
    }
}
