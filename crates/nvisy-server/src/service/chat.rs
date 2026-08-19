//! Assistant chat service.
//!
//! [`ChatService`] resolves a workspace's language-model connection into an
//! [`InferenceClient`], persists a session's messages (encrypting their content
//! under the workspace key), and drives a streaming chat turn against the
//! session's history.

use nvisy_inference::{ChatTurn, InferenceClient, TokenStream};
use nvisy_postgres::model::{ChatMessage, NewChatMessage};
use nvisy_postgres::query::{ChatMessageRepository, WorkspaceConnectionRepository};
use nvisy_postgres::types::{ChatRole, ProviderType};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::handler::{ErrorKind, Result};
use crate::service::{ConnectionConfig, Infra};

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

    /// Streams the assistant's reply to `prompt`, given the session's prior
    /// messages as context.
    ///
    /// Returns a [`TokenStream`] of text deltas; the caller persists the user
    /// prompt and the assembled reply around it. `history` is the session's
    /// stored messages in chronological order (with encrypted content).
    pub async fn stream_turn(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        history: &[ChatMessage],
        prompt: &str,
    ) -> Result<TokenStream> {
        let client = self.resolve_client(conn, workspace_id).await?;
        let history = self.to_history(workspace_id, history)?;
        Ok(client.stream_chat(prompt, history))
    }

    /// Appends a message to a session, encrypting its content under the
    /// workspace key. Returns the stored row.
    pub async fn append_message(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        session_id: Uuid,
        role: ChatRole,
        text: &str,
    ) -> Result<ChatMessage> {
        let content = self.infra.crypto.encrypt(workspace_id, text.as_bytes())?;
        Ok(conn
            .append_chat_message(NewChatMessage {
                session_id,
                role,
                content,
            })
            .await?)
    }

    /// Persists the assistant's assembled reply on its own pooled connection.
    ///
    /// Called after the stream completes (the request connection is already
    /// released), so it acquires a fresh connection.
    pub async fn persist_reply(
        &self,
        pg_client: &PgClient,
        workspace_id: Uuid,
        session_id: Uuid,
        reply: &str,
    ) -> Result<()> {
        let mut conn = pg_client.get_connection().await?;
        self.append_message(
            &mut conn,
            workspace_id,
            session_id,
            ChatRole::Assistant,
            reply,
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

    /// Builds the chat history from stored messages (decrypting each), preceded
    /// by the assistant preamble as a system instruction.
    fn to_history(&self, workspace_id: Uuid, messages: &[ChatMessage]) -> Result<Vec<ChatTurn>> {
        let mut history = Vec::with_capacity(messages.len() + 1);
        history.push(ChatTurn::system(PREAMBLE));
        for message in messages {
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
