//! Chat tool call repository for managing tool invocations within sessions.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{ChatToolCall, NewChatToolCall, UpdateChatToolCall};
use crate::types::{ChatToolStatus, CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for chat tool call database operations.
///
/// Handles tool invocation tracking including CRUD operations, status updates,
/// and querying by session, file, or status.
pub trait ChatToolCallRepository {
    /// Creates a new chat tool call.
    fn create_chat_tool_call(
        &mut self,
        tool_call: NewChatToolCall,
    ) -> impl Future<Output = PgResult<ChatToolCall>> + Send;

    /// Finds a chat tool call by its unique identifier.
    fn find_chat_tool_call_by_id(
        &mut self,
        tool_call_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ChatToolCall>>> + Send;

    /// Updates an existing chat tool call.
    fn update_chat_tool_call(
        &mut self,
        tool_call_id: Uuid,
        changes: UpdateChatToolCall,
    ) -> impl Future<Output = PgResult<ChatToolCall>> + Send;

    /// Deletes a chat tool call.
    fn delete_chat_tool_call(
        &mut self,
        tool_call_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists tool calls for a session with offset pagination.
    fn offset_list_session_tool_calls(
        &mut self,
        session_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<ChatToolCall>>> + Send;

    /// Lists tool calls for a session with cursor pagination.
    fn cursor_list_session_tool_calls(
        &mut self,
        session_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<ChatToolCall>>> + Send;

    /// Lists tool calls for a file with offset pagination.
    fn offset_list_file_tool_calls(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<ChatToolCall>>> + Send;

    /// Lists pending or running tool calls for a session.
    fn list_active_session_tool_calls(
        &mut self,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ChatToolCall>>> + Send;

    /// Updates the status of a tool call.
    fn update_chat_tool_call_status(
        &mut self,
        tool_call_id: Uuid,
        new_status: ChatToolStatus,
    ) -> impl Future<Output = PgResult<ChatToolCall>> + Send;

    /// Marks a tool call as completed with the given output.
    fn complete_chat_tool_call(
        &mut self,
        tool_call_id: Uuid,
        output: serde_json::Value,
    ) -> impl Future<Output = PgResult<ChatToolCall>> + Send;

    /// Cancels a pending or running tool call.
    fn cancel_chat_tool_call(
        &mut self,
        tool_call_id: Uuid,
    ) -> impl Future<Output = PgResult<ChatToolCall>> + Send;
}

impl ChatToolCallRepository for PgConnection {
    async fn create_chat_tool_call(
        &mut self,
        tool_call: NewChatToolCall,
    ) -> PgResult<ChatToolCall> {
        use schema::chat_tool_calls;

        let tool_call = diesel::insert_into(chat_tool_calls::table)
            .values(&tool_call)
            .returning(ChatToolCall::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(tool_call)
    }

    async fn find_chat_tool_call_by_id(
        &mut self,
        tool_call_id: Uuid,
    ) -> PgResult<Option<ChatToolCall>> {
        use schema::chat_tool_calls::dsl::*;

        let tool_call = chat_tool_calls
            .filter(id.eq(tool_call_id))
            .select(ChatToolCall::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(tool_call)
    }

    async fn update_chat_tool_call(
        &mut self,
        tool_call_id: Uuid,
        changes: UpdateChatToolCall,
    ) -> PgResult<ChatToolCall> {
        use schema::chat_tool_calls::dsl::*;

        let tool_call = diesel::update(chat_tool_calls)
            .filter(id.eq(tool_call_id))
            .set(&changes)
            .returning(ChatToolCall::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(tool_call)
    }

    async fn delete_chat_tool_call(&mut self, tool_call_id: Uuid) -> PgResult<()> {
        use schema::chat_tool_calls::dsl::*;

        diesel::delete(chat_tool_calls)
            .filter(id.eq(tool_call_id))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn offset_list_session_tool_calls(
        &mut self,
        sess_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<ChatToolCall>> {
        use schema::chat_tool_calls::{self, dsl};

        let tool_calls = chat_tool_calls::table
            .filter(dsl::session_id.eq(sess_id))
            .select(ChatToolCall::as_select())
            .order(dsl::started_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(tool_calls)
    }

    async fn cursor_list_session_tool_calls(
        &mut self,
        sess_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<ChatToolCall>> {
        use schema::chat_tool_calls::{self, dsl};

        let total = if pagination.include_count {
            Some(
                chat_tool_calls::table
                    .filter(dsl::session_id.eq(sess_id))
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let limit = pagination.limit + 1;

        let items: Vec<ChatToolCall> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            chat_tool_calls::table
                .filter(dsl::session_id.eq(sess_id))
                .filter(
                    dsl::started_at
                        .lt(&cursor_time)
                        .or(dsl::started_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(ChatToolCall::as_select())
                .order((dsl::started_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            chat_tool_calls::table
                .filter(dsl::session_id.eq(sess_id))
                .select(ChatToolCall::as_select())
                .order((dsl::started_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |tc: &ChatToolCall| (tc.started_at.into(), tc.id),
        ))
    }

    async fn offset_list_file_tool_calls(
        &mut self,
        f_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<ChatToolCall>> {
        use schema::chat_tool_calls::{self, dsl};

        let tool_calls = chat_tool_calls::table
            .filter(dsl::file_id.eq(f_id))
            .select(ChatToolCall::as_select())
            .order(dsl::started_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(tool_calls)
    }

    async fn list_active_session_tool_calls(
        &mut self,
        sess_id: Uuid,
    ) -> PgResult<Vec<ChatToolCall>> {
        use schema::chat_tool_calls::{self, dsl};

        let tool_calls = chat_tool_calls::table
            .filter(dsl::session_id.eq(sess_id))
            .filter(
                dsl::tool_status
                    .eq(ChatToolStatus::Pending)
                    .or(dsl::tool_status.eq(ChatToolStatus::Running)),
            )
            .select(ChatToolCall::as_select())
            .order(dsl::started_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(tool_calls)
    }

    async fn update_chat_tool_call_status(
        &mut self,
        tool_call_id: Uuid,
        new_status: ChatToolStatus,
    ) -> PgResult<ChatToolCall> {
        let changes = UpdateChatToolCall {
            tool_status: Some(new_status),
            ..Default::default()
        };

        self.update_chat_tool_call(tool_call_id, changes).await
    }

    async fn complete_chat_tool_call(
        &mut self,
        tool_call_id: Uuid,
        output: serde_json::Value,
    ) -> PgResult<ChatToolCall> {
        let changes = UpdateChatToolCall {
            tool_output: Some(output),
            tool_status: Some(ChatToolStatus::Completed),
            completed_at: Some(Some(jiff_diesel::Timestamp::from(jiff::Timestamp::now()))),
        };

        self.update_chat_tool_call(tool_call_id, changes).await
    }

    async fn cancel_chat_tool_call(&mut self, tool_call_id: Uuid) -> PgResult<ChatToolCall> {
        let changes = UpdateChatToolCall {
            tool_status: Some(ChatToolStatus::Cancelled),
            completed_at: Some(Some(jiff_diesel::Timestamp::from(jiff::Timestamp::now()))),
            ..Default::default()
        };

        self.update_chat_tool_call(tool_call_id, changes).await
    }
}
