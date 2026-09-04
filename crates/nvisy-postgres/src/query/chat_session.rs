//! Chat sessions repository.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{ChatSession, NewChatSession, UpdateChatSession};
use crate::types::{CursorPage, CursorPagination};
use crate::{Error, PgConnection, Result, schema};

/// Repository for chat session database operations.
pub trait ChatSessionRepository {
    /// Creates a new chat session.
    fn create_chat_session(
        &mut self,
        new_session: NewChatSession,
    ) -> impl Future<Output = Result<ChatSession>> + Send;

    /// Finds a live session by id within a workspace.
    fn find_chat_session_in_workspace(
        &mut self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> impl Future<Output = Result<Option<ChatSession>>> + Send;

    /// Lists a workspace's live sessions, newest first, cursor-paginated.
    ///
    /// Ordered by `(created_at, id)` — both immutable — so the cursor is stable
    /// even as sessions are updated during pagination.
    fn list_chat_sessions(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = Result<CursorPage<ChatSession>>> + Send;

    /// Updates a session (title and/or activity timestamp).
    fn update_chat_session(
        &mut self,
        session_id: Uuid,
        updates: UpdateChatSession,
    ) -> impl Future<Output = Result<ChatSession>> + Send;

    /// Soft-deletes a live session within a workspace, returning whether a live
    /// session was deleted.
    fn delete_chat_session(
        &mut self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> impl Future<Output = Result<bool>> + Send;
}

impl ChatSessionRepository for PgConnection {
    async fn create_chat_session(&mut self, new_session: NewChatSession) -> Result<ChatSession> {
        use schema::chat_sessions;

        diesel::insert_into(chat_sessions::table)
            .values(&new_session)
            .returning(ChatSession::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn find_chat_session_in_workspace(
        &mut self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<ChatSession>> {
        use schema::chat_sessions::{self, dsl};

        chat_sessions::table
            .filter(dsl::id.eq(session_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(ChatSession::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_chat_sessions(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> Result<CursorPage<ChatSession>> {
        use diesel::dsl::count_star;
        use schema::chat_sessions::{self, dsl};

        // Count only when the caller asked, so the default page skips the query.
        let total = if pagination.include_count {
            Some(
                chat_sessions::table
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .filter(dsl::deleted_at.is_null())
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let mut query = chat_sessions::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            query = query.filter(
                dsl::created_at
                    .lt(cursor_ts)
                    .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
            );
        }

        let sessions: Vec<ChatSession> = query
            .select(ChatSession::as_select())
            .order((dsl::created_at.desc(), dsl::id.desc()))
            .limit(pagination.fetch_limit())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(CursorPage::new(sessions, total, pagination.limit, |s| {
            (s.created_at.into(), s.id)
        }))
    }

    async fn update_chat_session(
        &mut self,
        session_id: Uuid,
        updates: UpdateChatSession,
    ) -> Result<ChatSession> {
        use schema::chat_sessions::{self, dsl};

        diesel::update(chat_sessions::table.filter(dsl::id.eq(session_id)))
            .set(updates)
            .returning(ChatSession::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn delete_chat_session(&mut self, workspace_id: Uuid, session_id: Uuid) -> Result<bool> {
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
        .map_err(Error::from)?;

        Ok(affected > 0)
    }
}
