//! Chat session repository for managing LLM-assisted editing sessions.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{ChatSession, NewChatSession, UpdateChatSession};
use crate::types::{ChatSessionStatus, CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for chat session database operations.
///
/// Handles LLM-assisted editing session management including CRUD operations,
/// status tracking, and usage statistics updates.
pub trait ChatSessionRepository {
    /// Creates a new chat session with the provided configuration.
    fn create_chat_session(
        &mut self,
        session: NewChatSession,
    ) -> impl Future<Output = PgResult<ChatSession>> + Send;

    /// Finds a chat session by its unique identifier.
    fn find_chat_session_by_id(
        &mut self,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ChatSession>>> + Send;

    /// Updates an existing chat session.
    fn update_chat_session(
        &mut self,
        session_id: Uuid,
        changes: UpdateChatSession,
    ) -> impl Future<Output = PgResult<ChatSession>> + Send;

    /// Deletes a chat session by archiving it.
    fn delete_chat_session(
        &mut self,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists chat sessions for a workspace with offset pagination.
    fn offset_list_chat_sessions(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<ChatSession>>> + Send;

    /// Lists chat sessions for a workspace with cursor pagination.
    fn cursor_list_chat_sessions(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<ChatSession>>> + Send;

    /// Lists chat sessions for an account with offset pagination.
    fn offset_list_account_chat_sessions(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<ChatSession>>> + Send;

    /// Lists active chat sessions for a file.
    fn list_file_chat_sessions(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ChatSession>>> + Send;

    /// Updates the status of a chat session.
    fn update_chat_session_status(
        &mut self,
        session_id: Uuid,
        new_status: ChatSessionStatus,
    ) -> impl Future<Output = PgResult<ChatSession>> + Send;

    /// Increments the message and token counts for a session.
    fn increment_chat_session_usage(
        &mut self,
        session_id: Uuid,
        messages: i32,
        tokens: i32,
    ) -> impl Future<Output = PgResult<ChatSession>> + Send;
}

impl ChatSessionRepository for PgConnection {
    async fn create_chat_session(&mut self, session: NewChatSession) -> PgResult<ChatSession> {
        use schema::chat_sessions;

        let session = diesel::insert_into(chat_sessions::table)
            .values(&session)
            .returning(ChatSession::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(session)
    }

    async fn find_chat_session_by_id(&mut self, session_id: Uuid) -> PgResult<Option<ChatSession>> {
        use schema::chat_sessions::dsl::*;

        let session = chat_sessions
            .filter(id.eq(session_id))
            .select(ChatSession::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(session)
    }

    async fn update_chat_session(
        &mut self,
        session_id: Uuid,
        changes: UpdateChatSession,
    ) -> PgResult<ChatSession> {
        use schema::chat_sessions::dsl::*;

        let session = diesel::update(chat_sessions)
            .filter(id.eq(session_id))
            .set(&changes)
            .returning(ChatSession::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(session)
    }

    async fn delete_chat_session(&mut self, session_id: Uuid) -> PgResult<()> {
        use schema::chat_sessions::dsl::*;

        diesel::update(chat_sessions)
            .filter(id.eq(session_id))
            .set(session_status.eq(ChatSessionStatus::Archived))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn offset_list_chat_sessions(
        &mut self,
        ws_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<ChatSession>> {
        use schema::chat_sessions::{self, dsl};

        let sessions = chat_sessions::table
            .filter(dsl::workspace_id.eq(ws_id))
            .select(ChatSession::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(sessions)
    }

    async fn cursor_list_chat_sessions(
        &mut self,
        ws_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<ChatSession>> {
        use schema::chat_sessions::{self, dsl};

        let total = if pagination.include_count {
            Some(
                chat_sessions::table
                    .filter(dsl::workspace_id.eq(ws_id))
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let limit = pagination.limit + 1;

        let items: Vec<ChatSession> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            chat_sessions::table
                .filter(dsl::workspace_id.eq(ws_id))
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(ChatSession::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            chat_sessions::table
                .filter(dsl::workspace_id.eq(ws_id))
                .select(ChatSession::as_select())
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
            |s: &ChatSession| (s.created_at.into(), s.id),
        ))
    }

    async fn offset_list_account_chat_sessions(
        &mut self,
        acc_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<ChatSession>> {
        use schema::chat_sessions::{self, dsl};

        let sessions = chat_sessions::table
            .filter(dsl::account_id.eq(acc_id))
            .select(ChatSession::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(sessions)
    }

    async fn list_file_chat_sessions(&mut self, file_id: Uuid) -> PgResult<Vec<ChatSession>> {
        use schema::chat_sessions::{self, dsl};

        let sessions = chat_sessions::table
            .filter(dsl::primary_file_id.eq(file_id))
            .filter(dsl::session_status.ne(ChatSessionStatus::Archived))
            .select(ChatSession::as_select())
            .order(dsl::created_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(sessions)
    }

    async fn update_chat_session_status(
        &mut self,
        session_id: Uuid,
        new_status: ChatSessionStatus,
    ) -> PgResult<ChatSession> {
        let changes = UpdateChatSession {
            session_status: Some(new_status),
            ..Default::default()
        };

        self.update_chat_session(session_id, changes).await
    }

    async fn increment_chat_session_usage(
        &mut self,
        session_id: Uuid,
        messages: i32,
        tokens: i32,
    ) -> PgResult<ChatSession> {
        use schema::chat_sessions::dsl::*;

        let session = diesel::update(chat_sessions)
            .filter(id.eq(session_id))
            .set((
                message_count.eq(message_count + messages),
                token_count.eq(token_count + tokens),
            ))
            .returning(ChatSession::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(session)
    }
}
