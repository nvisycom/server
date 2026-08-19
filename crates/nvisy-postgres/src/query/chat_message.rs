//! Chat messages repository.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{ChatMessage, NewChatMessage};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for chat message database operations.
pub trait ChatMessageRepository {
    /// Appends a message to its session and bumps the session's activity
    /// timestamp, in one transaction.
    fn append_chat_message(
        &mut self,
        new_message: NewChatMessage,
    ) -> impl Future<Output = PgResult<ChatMessage>> + Send;

    /// Loads a session's messages in chronological order (oldest first).
    fn list_chat_messages(
        &mut self,
        session_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ChatMessage>>> + Send;
}

impl ChatMessageRepository for PgConnection {
    async fn append_chat_message(&mut self, new_message: NewChatMessage) -> PgResult<ChatMessage> {
        use diesel::dsl::now;
        use diesel_async::AsyncConnection;
        use schema::{chat_messages, chat_sessions};

        // Insert the message and touch the session's `updated_at` atomically, so
        // the session-list ordering always reflects the latest message.
        self.transaction(async |conn| {
            let message = diesel::insert_into(chat_messages::table)
                .values(&new_message)
                .returning(ChatMessage::as_returning())
                .get_result(conn)
                .await
                .map_err(PgError::from)?;

            diesel::update(chat_sessions::table.filter(chat_sessions::id.eq(message.session_id)))
                .set(chat_sessions::updated_at.eq(now))
                .execute(conn)
                .await
                .map_err(PgError::from)?;

            Ok::<_, PgError>(message)
        })
        .await
    }

    async fn list_chat_messages(&mut self, session_id: Uuid) -> PgResult<Vec<ChatMessage>> {
        use schema::chat_messages::{self, dsl};

        chat_messages::table
            .filter(dsl::session_id.eq(session_id))
            .order(dsl::created_at.asc())
            .select(ChatMessage::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }
}
