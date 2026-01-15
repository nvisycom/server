//! Studio tool call repository for managing tool invocations within sessions.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewStudioToolCall, StudioToolCall, UpdateStudioToolCall};
use crate::types::{CursorPage, CursorPagination, OffsetPagination, StudioToolStatus};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for studio tool call database operations.
///
/// Handles tool invocation tracking including CRUD operations, status updates,
/// and querying by session, file, or status.
pub trait StudioToolCallRepository {
    /// Creates a new studio tool call.
    fn create_studio_tool_call(
        &mut self,
        tool_call: NewStudioToolCall,
    ) -> impl Future<Output = PgResult<StudioToolCall>> + Send;

    /// Finds a studio tool call by its unique identifier.
    fn find_studio_tool_call_by_id(
        &mut self,
        tool_call_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<StudioToolCall>>> + Send;

    /// Updates an existing studio tool call.
    fn update_studio_tool_call(
        &mut self,
        tool_call_id: Uuid,
        changes: UpdateStudioToolCall,
    ) -> impl Future<Output = PgResult<StudioToolCall>> + Send;

    /// Deletes a studio tool call.
    fn delete_studio_tool_call(
        &mut self,
        tool_call_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists tool calls for a session with offset pagination.
    fn offset_list_session_tool_calls(
        &mut self,
        session_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<StudioToolCall>>> + Send;

    /// Lists tool calls for a session with cursor pagination.
    fn cursor_list_session_tool_calls(
        &mut self,
        session_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<StudioToolCall>>> + Send;

    /// Lists tool calls for a file with offset pagination.
    fn offset_list_file_tool_calls(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<StudioToolCall>>> + Send;

    /// Lists pending or running tool calls for a session.
    fn list_active_session_tool_calls(
        &mut self,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<StudioToolCall>>> + Send;

    /// Updates the status of a tool call.
    fn update_studio_tool_call_status(
        &mut self,
        tool_call_id: Uuid,
        new_status: StudioToolStatus,
    ) -> impl Future<Output = PgResult<StudioToolCall>> + Send;

    /// Marks a tool call as completed with the given output.
    fn complete_studio_tool_call(
        &mut self,
        tool_call_id: Uuid,
        output: serde_json::Value,
    ) -> impl Future<Output = PgResult<StudioToolCall>> + Send;

    /// Cancels a pending or running tool call.
    fn cancel_studio_tool_call(
        &mut self,
        tool_call_id: Uuid,
    ) -> impl Future<Output = PgResult<StudioToolCall>> + Send;
}

impl StudioToolCallRepository for PgConnection {
    async fn create_studio_tool_call(
        &mut self,
        tool_call: NewStudioToolCall,
    ) -> PgResult<StudioToolCall> {
        use schema::studio_tool_calls;

        let tool_call = diesel::insert_into(studio_tool_calls::table)
            .values(&tool_call)
            .returning(StudioToolCall::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(tool_call)
    }

    async fn find_studio_tool_call_by_id(
        &mut self,
        tool_call_id: Uuid,
    ) -> PgResult<Option<StudioToolCall>> {
        use schema::studio_tool_calls::dsl::*;

        let tool_call = studio_tool_calls
            .filter(id.eq(tool_call_id))
            .select(StudioToolCall::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(tool_call)
    }

    async fn update_studio_tool_call(
        &mut self,
        tool_call_id: Uuid,
        changes: UpdateStudioToolCall,
    ) -> PgResult<StudioToolCall> {
        use schema::studio_tool_calls::dsl::*;

        let tool_call = diesel::update(studio_tool_calls)
            .filter(id.eq(tool_call_id))
            .set(&changes)
            .returning(StudioToolCall::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(tool_call)
    }

    async fn delete_studio_tool_call(&mut self, tool_call_id: Uuid) -> PgResult<()> {
        use schema::studio_tool_calls::dsl::*;

        diesel::delete(studio_tool_calls)
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
    ) -> PgResult<Vec<StudioToolCall>> {
        use schema::studio_tool_calls::{self, dsl};

        let tool_calls = studio_tool_calls::table
            .filter(dsl::session_id.eq(sess_id))
            .select(StudioToolCall::as_select())
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
    ) -> PgResult<CursorPage<StudioToolCall>> {
        use schema::studio_tool_calls::{self, dsl};

        let total = if pagination.include_count {
            Some(
                studio_tool_calls::table
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

        let items: Vec<StudioToolCall> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            studio_tool_calls::table
                .filter(dsl::session_id.eq(sess_id))
                .filter(
                    dsl::started_at
                        .lt(&cursor_time)
                        .or(dsl::started_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(StudioToolCall::as_select())
                .order((dsl::started_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            studio_tool_calls::table
                .filter(dsl::session_id.eq(sess_id))
                .select(StudioToolCall::as_select())
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
            |tc: &StudioToolCall| (tc.started_at.into(), tc.id),
        ))
    }

    async fn offset_list_file_tool_calls(
        &mut self,
        f_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<StudioToolCall>> {
        use schema::studio_tool_calls::{self, dsl};

        let tool_calls = studio_tool_calls::table
            .filter(dsl::file_id.eq(f_id))
            .select(StudioToolCall::as_select())
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
    ) -> PgResult<Vec<StudioToolCall>> {
        use schema::studio_tool_calls::{self, dsl};

        let tool_calls = studio_tool_calls::table
            .filter(dsl::session_id.eq(sess_id))
            .filter(
                dsl::tool_status
                    .eq(StudioToolStatus::Pending)
                    .or(dsl::tool_status.eq(StudioToolStatus::Running)),
            )
            .select(StudioToolCall::as_select())
            .order(dsl::started_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(tool_calls)
    }

    async fn update_studio_tool_call_status(
        &mut self,
        tool_call_id: Uuid,
        new_status: StudioToolStatus,
    ) -> PgResult<StudioToolCall> {
        let changes = UpdateStudioToolCall {
            tool_status: Some(new_status),
            ..Default::default()
        };

        self.update_studio_tool_call(tool_call_id, changes).await
    }

    async fn complete_studio_tool_call(
        &mut self,
        tool_call_id: Uuid,
        output: serde_json::Value,
    ) -> PgResult<StudioToolCall> {
        let changes = UpdateStudioToolCall {
            tool_output: Some(output),
            tool_status: Some(StudioToolStatus::Completed),
            completed_at: Some(Some(jiff_diesel::Timestamp::from(jiff::Timestamp::now()))),
        };

        self.update_studio_tool_call(tool_call_id, changes).await
    }

    async fn cancel_studio_tool_call(&mut self, tool_call_id: Uuid) -> PgResult<StudioToolCall> {
        let changes = UpdateStudioToolCall {
            tool_status: Some(StudioToolStatus::Cancelled),
            completed_at: Some(Some(jiff_diesel::Timestamp::from(jiff::Timestamp::now()))),
            ..Default::default()
        };

        self.update_studio_tool_call(tool_call_id, changes).await
    }
}
