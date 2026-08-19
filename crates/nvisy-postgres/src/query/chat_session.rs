//! Chat sessions repository.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{ChatSession, NewChatSession, UpdateChatSession};
use crate::types::OffsetPagination;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for chat session database operations.
pub trait ChatSessionRepository {
    /// Creates a new chat session.
    fn create_chat_session(
        &mut self,
        new_session: NewChatSession,
    ) -> impl Future<Output = PgResult<ChatSession>> + Send;

    /// Finds a live session by id within a workspace.
    fn find_chat_session_in_workspace(
        &mut self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ChatSession>>> + Send;

    /// Lists a workspace's live sessions, most recently active first.
    fn list_chat_sessions(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<ChatSession>>> + Send;

    /// Updates a session (title and/or activity timestamp).
    fn update_chat_session(
        &mut self,
        session_id: Uuid,
        updates: UpdateChatSession,
    ) -> impl Future<Output = PgResult<ChatSession>> + Send;

    /// Soft-deletes a live session within a workspace, returning whether a live
    /// session was deleted.
    fn delete_chat_session(
        &mut self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

impl ChatSessionRepository for PgConnection {
    async fn create_chat_session(&mut self, new_session: NewChatSession) -> PgResult<ChatSession> {
        use schema::chat_sessions;

        diesel::insert_into(chat_sessions::table)
            .values(&new_session)
            .returning(ChatSession::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_chat_session_in_workspace(
        &mut self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> PgResult<Option<ChatSession>> {
        use schema::chat_sessions::{self, dsl};

        chat_sessions::table
            .filter(dsl::id.eq(session_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(ChatSession::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn list_chat_sessions(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<ChatSession>> {
        use schema::chat_sessions::{self, dsl};

        chat_sessions::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ChatSession::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn update_chat_session(
        &mut self,
        session_id: Uuid,
        updates: UpdateChatSession,
    ) -> PgResult<ChatSession> {
        use schema::chat_sessions::{self, dsl};

        diesel::update(chat_sessions::table.filter(dsl::id.eq(session_id)))
            .set(updates)
            .returning(ChatSession::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn delete_chat_session(
        &mut self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> PgResult<bool> {
        use diesel::dsl::now;
        use schema::chat_sessions::{self, dsl};

        let affected = diesel::update(
            chat_sessions::table
                .filter(dsl::id.eq(session_id))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(now))
        .execute(self)
        .await
        .map_err(PgError::from)?;

        Ok(affected > 0)
    }
}
