//! Chat messages repository.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{ChatMessage, NewChatMessage, UpdateChatSession};
use crate::{Error, PgConnection, Result, schema};

/// What to update on a message's session when appending it, applied in the same
/// transaction as the insert so the message and its session state never diverge.
#[derive(Debug, Clone, Default)]
pub struct AppendSessionUpdate {
    /// Point the session's active leaf at the newly appended message.
    pub advance_leaf: bool,
    /// Set the session's title (e.g. seeded from the first message).
    pub title: Option<String>,
}

/// Repository for chat message database operations.
pub trait ChatMessageRepository {
    /// Appends a message and updates its session per `session_update` in one
    /// transaction, so the session's active leaf and title never diverge from
    /// its messages. The session's `updated_at` is always bumped. Returns the
    /// stored message.
    fn append_chat_message(
        &mut self,
        new_message: NewChatMessage,
        session_update: AppendSessionUpdate,
    ) -> impl Future<Output = Result<ChatMessage>> + Send;

    /// Loads all of a session's messages (the whole tree), oldest first.
    fn list_chat_messages(
        &mut self,
        session_id: Uuid,
    ) -> impl Future<Output = Result<Vec<ChatMessage>>> + Send;

    /// Finds a message by id within a session, scoping a client-supplied parent
    /// to the session it belongs to.
    fn find_chat_message_in_session(
        &mut self,
        session_id: Uuid,
        message_id: Uuid,
    ) -> impl Future<Output = Result<Option<ChatMessage>>> + Send;
}

impl ChatMessageRepository for PgConnection {
    async fn append_chat_message(
        &mut self,
        new_message: NewChatMessage,
        session_update: AppendSessionUpdate,
    ) -> Result<ChatMessage> {
        use diesel::dsl::now;
        use diesel_async::AsyncConnection;
        use schema::{chat_messages, chat_sessions};

        let session_id = new_message.session_id;

        // Insert the message and update its session atomically, so the active
        // leaf and title never diverge from the messages. `updated_at` is always
        // bumped so the session-list ordering reflects the latest message. The
        // active leaf is set to the row just inserted (its id is known only here).
        self.transaction(async |conn| {
            let message = diesel::insert_into(chat_messages::table)
                .values(&new_message)
                .returning(ChatMessage::as_returning())
                .get_result(conn)
                .await
                .map_err(Error::from)?;

            let update = UpdateChatSession {
                title: session_update.title,
                current_message_id: session_update.advance_leaf.then_some(Some(message.id)),
                updated_at: None,
            };
            diesel::update(chat_sessions::table.filter(chat_sessions::id.eq(session_id)))
                .set((update, chat_sessions::updated_at.eq(now)))
                .execute(conn)
                .await
                .map_err(Error::from)?;

            Ok::<_, Error>(message)
        })
        .await
    }

    async fn list_chat_messages(&mut self, session_id: Uuid) -> Result<Vec<ChatMessage>> {
        use schema::chat_messages::{self, dsl};

        chat_messages::table
            .filter(dsl::session_id.eq(session_id))
            .order(dsl::created_at.asc())
            .select(ChatMessage::as_select())
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn find_chat_message_in_session(
        &mut self,
        session_id: Uuid,
        message_id: Uuid,
    ) -> Result<Option<ChatMessage>> {
        use schema::chat_messages::{self, dsl};

        chat_messages::table
            .filter(dsl::id.eq(message_id))
            .filter(dsl::session_id.eq(session_id))
            .select(ChatMessage::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
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
