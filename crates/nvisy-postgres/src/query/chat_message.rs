//! Chat messages repository.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{ChatMessage, NewChatMessage};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for chat message database operations.
pub trait ChatMessageRepository {
    /// Appends a message and bumps its session's activity timestamp, in one
    /// transaction.
    fn append_chat_message(
        &mut self,
        new_message: NewChatMessage,
    ) -> impl Future<Output = PgResult<ChatMessage>> + Send;

    /// Loads all of a session's messages (the whole tree), oldest first.
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

        let session_id = new_message.session_id;

        // Insert the message and touch the session's `updated_at` atomically, so
        // the session-list ordering always reflects the latest message.
        self.transaction(async |conn| {
            let message = diesel::insert_into(chat_messages::table)
                .values(&new_message)
                .returning(ChatMessage::as_returning())
                .get_result(conn)
                .await
                .map_err(PgError::from)?;

            diesel::update(chat_sessions::table.filter(chat_sessions::id.eq(session_id)))
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

impl ChatMessage {
    /// The active conversation path ending at `leaf_id`: the chain of messages
    /// from the root down to that leaf, in chronological order.
    ///
    /// Follows `parent_id` links up from the leaf through `messages` (the
    /// session's full message set), then reverses. A `None` leaf, or a leaf not
    /// present, yields an empty path. Sessions are small, so walking the loaded
    /// set in memory is cheaper and simpler than a recursive query.
    #[must_use]
    pub fn path_to(messages: &[ChatMessage], leaf_id: Option<Uuid>) -> Vec<&ChatMessage> {
        use std::collections::HashMap;

        let by_id: HashMap<Uuid, &ChatMessage> = messages.iter().map(|m| (m.id, m)).collect();

        let mut path = Vec::new();
        let mut cursor = leaf_id;
        while let Some(id) = cursor {
            let Some(message) = by_id.get(&id) else { break };
            path.push(*message);
            cursor = message.parent_id;
        }
        path.reverse();
        path
    }
}
