//! Studio session repository for managing LLM-assisted editing sessions.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewStudioSession, StudioSession, UpdateStudioSession};
use crate::types::{CursorPage, CursorPagination, OffsetPagination, StudioSessionStatus};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for studio session database operations.
///
/// Handles LLM-assisted editing session management including CRUD operations,
/// status tracking, and usage statistics updates.
pub trait StudioSessionRepository {
    /// Creates a new studio session with the provided configuration.
    fn create_studio_session(
        &mut self,
        session: NewStudioSession,
    ) -> impl Future<Output = PgResult<StudioSession>> + Send;

    /// Finds a studio session by its unique identifier.
    fn find_studio_session_by_id(
        &mut self,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<StudioSession>>> + Send;

    /// Updates an existing studio session.
    fn update_studio_session(
        &mut self,
        session_id: Uuid,
        changes: UpdateStudioSession,
    ) -> impl Future<Output = PgResult<StudioSession>> + Send;

    /// Deletes a studio session by archiving it.
    fn delete_studio_session(
        &mut self,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists studio sessions for a workspace with offset pagination.
    fn offset_list_studio_sessions(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<StudioSession>>> + Send;

    /// Lists studio sessions for a workspace with cursor pagination.
    fn cursor_list_studio_sessions(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<StudioSession>>> + Send;

    /// Lists studio sessions for an account with offset pagination.
    fn offset_list_account_studio_sessions(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<StudioSession>>> + Send;

    /// Lists active studio sessions for a file.
    fn list_file_studio_sessions(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<StudioSession>>> + Send;

    /// Updates the status of a studio session.
    fn update_studio_session_status(
        &mut self,
        session_id: Uuid,
        new_status: StudioSessionStatus,
    ) -> impl Future<Output = PgResult<StudioSession>> + Send;

    /// Increments the message and token counts for a session.
    fn increment_studio_session_usage(
        &mut self,
        session_id: Uuid,
        messages: i32,
        tokens: i32,
    ) -> impl Future<Output = PgResult<StudioSession>> + Send;
}

impl StudioSessionRepository for PgConnection {
    async fn create_studio_session(
        &mut self,
        session: NewStudioSession,
    ) -> PgResult<StudioSession> {
        use schema::studio_sessions;

        let session = diesel::insert_into(studio_sessions::table)
            .values(&session)
            .returning(StudioSession::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(session)
    }

    async fn find_studio_session_by_id(
        &mut self,
        session_id: Uuid,
    ) -> PgResult<Option<StudioSession>> {
        use schema::studio_sessions::dsl::*;

        let session = studio_sessions
            .filter(id.eq(session_id))
            .select(StudioSession::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(session)
    }

    async fn update_studio_session(
        &mut self,
        session_id: Uuid,
        changes: UpdateStudioSession,
    ) -> PgResult<StudioSession> {
        use schema::studio_sessions::dsl::*;

        let session = diesel::update(studio_sessions)
            .filter(id.eq(session_id))
            .set(&changes)
            .returning(StudioSession::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(session)
    }

    async fn delete_studio_session(&mut self, session_id: Uuid) -> PgResult<()> {
        use schema::studio_sessions::dsl::*;

        diesel::update(studio_sessions)
            .filter(id.eq(session_id))
            .set(session_status.eq(StudioSessionStatus::Archived))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn offset_list_studio_sessions(
        &mut self,
        ws_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<StudioSession>> {
        use schema::studio_sessions::{self, dsl};

        let sessions = studio_sessions::table
            .filter(dsl::workspace_id.eq(ws_id))
            .select(StudioSession::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(sessions)
    }

    async fn cursor_list_studio_sessions(
        &mut self,
        ws_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<StudioSession>> {
        use schema::studio_sessions::{self, dsl};

        let total = if pagination.include_count {
            Some(
                studio_sessions::table
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

        let items: Vec<StudioSession> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            studio_sessions::table
                .filter(dsl::workspace_id.eq(ws_id))
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(StudioSession::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            studio_sessions::table
                .filter(dsl::workspace_id.eq(ws_id))
                .select(StudioSession::as_select())
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
            |s: &StudioSession| (s.created_at.into(), s.id),
        ))
    }

    async fn offset_list_account_studio_sessions(
        &mut self,
        acc_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<StudioSession>> {
        use schema::studio_sessions::{self, dsl};

        let sessions = studio_sessions::table
            .filter(dsl::account_id.eq(acc_id))
            .select(StudioSession::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(sessions)
    }

    async fn list_file_studio_sessions(&mut self, file_id: Uuid) -> PgResult<Vec<StudioSession>> {
        use schema::studio_sessions::{self, dsl};

        let sessions = studio_sessions::table
            .filter(dsl::primary_file_id.eq(file_id))
            .filter(dsl::session_status.ne(StudioSessionStatus::Archived))
            .select(StudioSession::as_select())
            .order(dsl::created_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(sessions)
    }

    async fn update_studio_session_status(
        &mut self,
        session_id: Uuid,
        new_status: StudioSessionStatus,
    ) -> PgResult<StudioSession> {
        let changes = UpdateStudioSession {
            session_status: Some(new_status),
            ..Default::default()
        };

        self.update_studio_session(session_id, changes).await
    }

    async fn increment_studio_session_usage(
        &mut self,
        session_id: Uuid,
        messages: i32,
        tokens: i32,
    ) -> PgResult<StudioSession> {
        use schema::studio_sessions::dsl::*;

        let session = diesel::update(studio_sessions)
            .filter(id.eq(session_id))
            .set((
                message_count.eq(message_count + messages),
                token_count.eq(token_count + tokens),
            ))
            .returning(StudioSession::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(session)
    }
}
