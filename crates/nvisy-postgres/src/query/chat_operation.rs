//! Chat operation repository for managing document operations (diffs).

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{ChatOperation, NewChatOperation, UpdateChatOperation};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for chat operation database operations.
///
/// Handles document operation tracking including CRUD operations, apply/revert
/// state management, and querying by tool call or file.
pub trait ChatOperationRepository {
    /// Creates a new chat operation.
    fn create_chat_operation(
        &mut self,
        operation: NewChatOperation,
    ) -> impl Future<Output = PgResult<ChatOperation>> + Send;

    /// Creates multiple chat operations in a batch.
    fn create_chat_operations(
        &mut self,
        operations: Vec<NewChatOperation>,
    ) -> impl Future<Output = PgResult<Vec<ChatOperation>>> + Send;

    /// Finds a chat operation by its unique identifier.
    fn find_chat_operation_by_id(
        &mut self,
        operation_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ChatOperation>>> + Send;

    /// Updates an existing chat operation.
    fn update_chat_operation(
        &mut self,
        operation_id: Uuid,
        changes: UpdateChatOperation,
    ) -> impl Future<Output = PgResult<ChatOperation>> + Send;

    /// Deletes a chat operation.
    fn delete_chat_operation(
        &mut self,
        operation_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists operations for a tool call.
    fn list_tool_call_operations(
        &mut self,
        tool_call_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ChatOperation>>> + Send;

    /// Lists operations for a file with offset pagination.
    fn offset_list_file_operations(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<ChatOperation>>> + Send;

    /// Lists operations for a file with cursor pagination.
    fn cursor_list_file_operations(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<ChatOperation>>> + Send;

    /// Lists pending (unapplied) operations for a file.
    fn list_pending_file_operations(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ChatOperation>>> + Send;

    /// Marks an operation as applied.
    fn apply_chat_operation(
        &mut self,
        operation_id: Uuid,
    ) -> impl Future<Output = PgResult<ChatOperation>> + Send;

    /// Marks multiple operations as applied.
    fn apply_chat_operations(
        &mut self,
        operation_ids: Vec<Uuid>,
    ) -> impl Future<Output = PgResult<Vec<ChatOperation>>> + Send;

    /// Marks an operation as reverted.
    fn revert_chat_operation(
        &mut self,
        operation_id: Uuid,
    ) -> impl Future<Output = PgResult<ChatOperation>> + Send;

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

impl ChatOperationRepository for PgConnection {
    async fn create_chat_operation(
        &mut self,
        operation: NewChatOperation,
    ) -> PgResult<ChatOperation> {
        use schema::chat_operations;

        let operation = diesel::insert_into(chat_operations::table)
            .values(&operation)
            .returning(ChatOperation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(operation)
    }

    async fn create_chat_operations(
        &mut self,
        operations: Vec<NewChatOperation>,
    ) -> PgResult<Vec<ChatOperation>> {
        use schema::chat_operations;

        let operations = diesel::insert_into(chat_operations::table)
            .values(&operations)
            .returning(ChatOperation::as_returning())
            .get_results(self)
            .await
            .map_err(PgError::from)?;

        Ok(operations)
    }

    async fn find_chat_operation_by_id(
        &mut self,
        operation_id: Uuid,
    ) -> PgResult<Option<ChatOperation>> {
        use schema::chat_operations::dsl::*;

        let operation = chat_operations
            .filter(id.eq(operation_id))
            .select(ChatOperation::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(operation)
    }

    async fn update_chat_operation(
        &mut self,
        operation_id: Uuid,
        changes: UpdateChatOperation,
    ) -> PgResult<ChatOperation> {
        use schema::chat_operations::dsl::*;

        let operation = diesel::update(chat_operations)
            .filter(id.eq(operation_id))
            .set(&changes)
            .returning(ChatOperation::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(operation)
    }

    async fn delete_chat_operation(&mut self, operation_id: Uuid) -> PgResult<()> {
        use schema::chat_operations::dsl::*;

        diesel::delete(chat_operations)
            .filter(id.eq(operation_id))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn list_tool_call_operations(&mut self, tc_id: Uuid) -> PgResult<Vec<ChatOperation>> {
        use schema::chat_operations::{self, dsl};

        let operations = chat_operations::table
            .filter(dsl::tool_call_id.eq(tc_id))
            .select(ChatOperation::as_select())
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
    ) -> PgResult<Vec<ChatOperation>> {
        use schema::chat_operations::{self, dsl};

        let operations = chat_operations::table
            .filter(dsl::file_id.eq(f_id))
            .select(ChatOperation::as_select())
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
    ) -> PgResult<CursorPage<ChatOperation>> {
        use schema::chat_operations::{self, dsl};

        let total = if pagination.include_count {
            Some(
                chat_operations::table
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

        let items: Vec<ChatOperation> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            chat_operations::table
                .filter(dsl::file_id.eq(f_id))
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(ChatOperation::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            chat_operations::table
                .filter(dsl::file_id.eq(f_id))
                .select(ChatOperation::as_select())
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
            |op: &ChatOperation| (op.created_at.into(), op.id),
        ))
    }

    async fn list_pending_file_operations(&mut self, f_id: Uuid) -> PgResult<Vec<ChatOperation>> {
        use schema::chat_operations::{self, dsl};

        let operations = chat_operations::table
            .filter(dsl::file_id.eq(f_id))
            .filter(dsl::applied.eq(false))
            .select(ChatOperation::as_select())
            .order(dsl::created_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(operations)
    }

    async fn apply_chat_operation(&mut self, operation_id: Uuid) -> PgResult<ChatOperation> {
        let changes = UpdateChatOperation {
            applied: Some(true),
            applied_at: Some(Some(jiff_diesel::Timestamp::from(jiff::Timestamp::now()))),
            ..Default::default()
        };

        self.update_chat_operation(operation_id, changes).await
    }

    async fn apply_chat_operations(
        &mut self,
        operation_ids: Vec<Uuid>,
    ) -> PgResult<Vec<ChatOperation>> {
        use schema::chat_operations::dsl::*;

        let now = jiff_diesel::Timestamp::from(jiff::Timestamp::now());

        let operations = diesel::update(chat_operations)
            .filter(id.eq_any(&operation_ids))
            .set((applied.eq(true), applied_at.eq(Some(now))))
            .returning(ChatOperation::as_returning())
            .get_results(self)
            .await
            .map_err(PgError::from)?;

        Ok(operations)
    }

    async fn revert_chat_operation(&mut self, operation_id: Uuid) -> PgResult<ChatOperation> {
        let changes = UpdateChatOperation {
            reverted: Some(true),
            ..Default::default()
        };

        self.update_chat_operation(operation_id, changes).await
    }

    async fn count_file_operations(&mut self, f_id: Uuid) -> PgResult<FileOperationCounts> {
        use diesel::dsl::count_star;
        use schema::chat_operations::{self, dsl};

        let total = chat_operations::table
            .filter(dsl::file_id.eq(f_id))
            .select(count_star())
            .get_result::<i64>(self)
            .await
            .map_err(PgError::from)?;

        let applied_count = chat_operations::table
            .filter(dsl::file_id.eq(f_id))
            .filter(dsl::applied.eq(true))
            .select(count_star())
            .get_result::<i64>(self)
            .await
            .map_err(PgError::from)?;

        let reverted_count = chat_operations::table
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
