//! Assistant chat service.
//!
//! [`ChatService`] resolves a workspace's language-model connection into an
//! [`InferenceClient`], persists a session's messages (encrypting their content
//! under the workspace key), and drives a streaming chat turn against the
//! session's history.

use nvisy_inference::{ChatTurn, InferenceClient, TokenStream};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{ChatMessage, NewChatMessage};
use nvisy_postgres::query::{
    AppendSessionUpdate, ChatMessageRepository, WorkspaceConnectionRepository,
};
use nvisy_postgres::types::{ChatRole, ProviderType};
use uuid::Uuid;

use crate::handler::{ErrorKind, Result};
use crate::service::{ConnectionConfig, Infra};

/// Where in a conversation a turn happens: the workspace and session it belongs
/// to, and the message it extends (its parent in the tree; `None` starts a new
/// root).
#[derive(Debug, Clone, Copy)]
pub struct TurnLocation {
    /// Workspace owning the session (and its encryption key + model connection).
    pub workspace_id: Uuid,
    /// Session the turn belongs to.
    pub session_id: Uuid,
    /// The message this turn replies to; `None` is a root.
    pub parent_id: Option<Uuid>,
}

/// The assistant's system preamble. Kept deliberately narrow: this is a plain
/// chat assistant with no access to document contents (a hard constraint on a
/// redaction platform).
const PREAMBLE: &str = "You are the assistant for a document redaction platform. \
     Help the user understand and operate their workspace: redaction policies, \
     detections, and pipelines. You do not have access to the contents of any \
     document. Be concise and accurate.";

/// Resolves a workspace's inference backend and drives streaming chat turns.
///
/// Cloneable and cheap to pass around: holds the shared [`Infra`] clients (all
/// `Arc`-backed) and takes the per-request database connection as a method
/// argument.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct ChatService {
    infra: Infra,
}

impl ChatService {
    /// Creates a new [`ChatService`].
    pub fn new(infra: Infra) -> Self {
        Self { infra }
    }

    /// Resolves the workspace's language-model connection into an inference
    /// client.
    ///
    /// Errors when the workspace has no language-model connection configured
    /// (`409 Conflict`), or when its stored config is not an inference config or
    /// cannot build a client (`500`).
    async fn resolve_client(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
    ) -> Result<InferenceClient> {
        let connection = conn
            .find_connection_by_type(workspace_id, ProviderType::LanguageModel)
            .await?
            .ok_or_else(|| {
                ErrorKind::Conflict
                    .with_message("This workspace has no language model connection configured")
                    .with_resource("connection")
            })?;

        let config: ConnectionConfig = self
            .infra
            .crypto
            .decrypt_json(workspace_id, &connection.encrypted_data)?;

        let ConnectionConfig::Inference(llm) = config else {
            return Err(ErrorKind::InternalServerError
                .with_message("Connection is not a language model connection"));
        };

        llm.connect(None).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to build the language model client")
                .with_context(err.to_string())
        })
    }

    /// Streams the assistant's reply to `prompt`, using the conversation path
    /// ending at `parent_id` as context.
    ///
    /// Loads the session's messages, walks the path (root → `parent_id`),
    /// decrypts it, and streams the model's reply. Resolving the model connection
    /// happens first, so a missing connection fails before the caller persists
    /// anything. Returns a [`TokenStream`] of text deltas.
    pub async fn stream_turn(
        &self,
        conn: &mut PgConn,
        at: TurnLocation,
        prompt: &str,
    ) -> Result<TokenStream> {
        let client = self.resolve_client(conn, at.workspace_id).await?;
        let messages = conn.list_chat_messages(at.session_id).await?;
        let path = ChatMessage::path_to(&messages, at.parent_id);
        let history = self.to_history(at.workspace_id, &path)?;
        Ok(client.stream_chat(prompt, history))
    }

    /// Appends a message at `at` in the session's tree — encrypting its content
    /// under the workspace key — and applies `session_update` (advance the active
    /// leaf, set the title) in the same transaction, so a message and the session
    /// state it implies never diverge. Returns the stored row.
    pub async fn append_message(
        &self,
        conn: &mut PgConn,
        at: TurnLocation,
        role: ChatRole,
        text: &str,
        session_update: AppendSessionUpdate,
    ) -> Result<ChatMessage> {
        let content = self
            .infra
            .crypto
            .encrypt(at.workspace_id, text.as_bytes())?;
        Ok(conn
            .append_chat_message(
                NewChatMessage {
                    session_id: at.session_id,
                    parent_id: at.parent_id,
                    role,
                    content,
                },
                session_update,
            )
            .await?)
    }

    /// Persists the assistant's assembled reply at `at`, advancing the session's
    /// active leaf to it in the same transaction.
    ///
    /// Acquires its own connection: it runs after the stream completes, when the
    /// request connection has already been released back to the pool.
    pub async fn persist_reply(&self, at: TurnLocation, reply: &str) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        self.append_message(
            &mut conn,
            at,
            ChatRole::Assistant,
            reply,
            AppendSessionUpdate {
                advance_leaf: true,
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    /// Decrypts a stored message's content under the workspace key.
    pub fn decrypt_content(&self, workspace_id: Uuid, message: &ChatMessage) -> Result<String> {
        let bytes = self.infra.crypto.decrypt(workspace_id, &message.content)?;
        String::from_utf8(bytes).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Stored chat message is not valid UTF-8")
                .with_context(err.to_string())
        })
    }

    /// Builds the chat history from a decrypted path, preceded by the assistant
    /// preamble as a system instruction.
    fn to_history(&self, workspace_id: Uuid, path: &[&ChatMessage]) -> Result<Vec<ChatTurn>> {
        let mut history = Vec::with_capacity(path.len() + 1);
        history.push(ChatTurn::system(PREAMBLE));
        for message in path {
            let content = self.decrypt_content(workspace_id, message)?;
            history.push(match message.role {
                ChatRole::System => ChatTurn::system(content),
                ChatRole::User => ChatTurn::user(content),
                ChatRole::Assistant => ChatTurn::assistant(content),
            });
        }
        Ok(history)
    }
}
