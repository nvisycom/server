//! Assistant chat handlers: sessions and streaming messages.
//!
//! Chat is a workspace-scoped assistant. A session is a thread of messages; a
//! message POST persists the user's turn, streams the model's reply over SSE,
//! and persists the assembled reply when the stream ends. The model is the
//! workspace's language-model connection; it has no access to document contents.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use async_stream::stream;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::Event;
use futures::StreamExt;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::NewChatSession;
use nvisy_postgres::query::{AppendSessionUpdate, ChatMessageRepository, ChatSessionRepository};
use nvisy_postgres::types::ChatRole;
use tokio_util::sync::CancellationToken;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson, WorkspaceContext,
};
use crate::handler::request::{
    ChatSessionPathParams, CreateChatSession, CursorPagination, SendChatMessage,
};
use crate::handler::response::{ChatMessage, ChatSession, ChatSessionsPage, ErrorResponse};
use crate::handler::utility::SseResponse;
use crate::handler::{Error, Result};
use crate::service::{ChatService, ServiceState, TurnLocation};

/// Tracing target for chat operations.
const TRACING_TARGET: &str = "nvisy_server::handler::chat";

/// How long a session title seeded from the first message may be.
const TITLE_MAX: usize = 80;

/// The default session title, until seeded from the first message.
const DEFAULT_TITLE: &str = "New chat";

/// Maximum assistant-reply length, in bytes of plaintext. Kept below the
/// encrypted-content column limit (131072 bytes) with headroom for the
/// encryption framing (nonce, tag, chunking), so an accepted reply always fits.
const MAX_REPLY_BYTES: usize = 96 * 1024;

/// Creates a new chat session in the workspace.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id, workspace_id = %workspace.id))]
async fn create_session(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    ValidateJson(request): ValidateJson<CreateChatSession>,
) -> Result<(StatusCode, Json<ChatSession>)> {
    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    let session = conn
        .create_chat_session(NewChatSession {
            workspace_id: workspace.id,
            account_id: auth_state.account_id,
            title: request.title.unwrap_or_else(|| DEFAULT_TITLE.to_owned()),
        })
        .await?;

    tracing::info!(target: TRACING_TARGET, session_id = %session.id, "Chat session created");
    Ok((StatusCode::CREATED, Json(ChatSession::from_model(session))))
}

fn create_session_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create chat session")
        .description("Opens a new assistant chat session in the workspace.")
        .response::<201, Json<ChatSession>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists the workspace's chat sessions, most recently active first.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id, workspace_id = %workspace.id))]
async fn list_sessions(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<ChatSessionsPage>)> {
    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    let page = conn
        .list_chat_sessions(workspace.id, pagination.into())
        .await?;
    let response = ChatSessionsPage::from_cursor_page(page, ChatSession::from_model);

    Ok((StatusCode::OK, Json(response)))
}

fn list_sessions_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List chat sessions")
        .description("Returns the workspace's chat sessions, newest first, cursor-paginated.")
        .response::<200, Json<ChatSessionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Returns a session's messages in chronological order.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id, workspace_id = %workspace.id, session_id = %path_params.session_id))]
async fn list_messages(
    State(pg_client): State<PgClient>,
    State(chat): State<ChatService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ChatSessionPathParams>,
) -> Result<(StatusCode, Json<Vec<ChatMessage>>)> {
    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    // Scope the session to the workspace before reading its messages.
    conn.find_chat_session_in_workspace(workspace.id, path_params.session_id)
        .await?
        .ok_or_else(|| Error::not_found("chat session"))?;

    let messages = conn.list_chat_messages(path_params.session_id).await?;
    let items = messages
        .into_iter()
        .map(|message| ChatMessage::from_model(message, workspace.id, &chat))
        .collect::<Result<Vec<_>>>()?;

    Ok((StatusCode::OK, Json(items)))
}

fn list_messages_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List chat messages")
        .description("Returns a session's messages in chronological order.")
        .response::<200, Json<Vec<ChatMessage>>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a chat session.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id, workspace_id = %workspace.id, session_id = %path_params.session_id))]
async fn delete_session(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ChatSessionPathParams>,
) -> Result<StatusCode> {
    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    let deleted = conn
        .delete_chat_session(workspace.id, path_params.session_id)
        .await?;
    if !deleted {
        return Err(Error::not_found("chat session"));
    }

    Ok(StatusCode::NO_CONTENT)
}

fn delete_session_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete chat session")
        .description("Soft-deletes a chat session.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Sends a message and streams the assistant's reply as Server-Sent Events.
///
/// Persists the user message, streams the model's reply as `token` events, and
/// persists the assembled reply when the stream ends. Empty replies (the model
/// produced no text) are not persisted.
///
/// Authenticated with a Bearer token; browsers should consume it via a `fetch`
/// stream rather than the native `EventSource`, which cannot send an
/// `Authorization` header.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id, workspace_id = %workspace.id, session_id = %path_params.session_id))]
async fn send_message(
    State(pg_client): State<PgClient>,
    State(chat): State<ChatService>,
    State(shutdown): State<CancellationToken>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ChatSessionPathParams>,
    ValidateJson(request): ValidateJson<SendChatMessage>,
) -> Result<SseResponse<ChatToken>> {
    let session_id = path_params.session_id;
    let workspace_id = workspace.id;

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace_id, Permission::ViewWorkspace)
        .await?;

    // Scope the session to the workspace before writing to it.
    let session = conn
        .find_chat_session_in_workspace(workspace_id, session_id)
        .await?
        .ok_or_else(|| Error::not_found("chat session"))?;

    // An explicit parent must belong to this session. The composite FK enforces
    // this at write time, but reject it here — before inference — for a clean 404
    // rather than a failed insert after the model has run.
    if let Some(parent_id) = request.parent_id
        && conn
            .find_chat_message_in_session(session_id, parent_id)
            .await?
            .is_none()
    {
        return Err(Error::not_found("chat message"));
    }

    // The turn extends the branch the client is on: an explicit parent, else the
    // session's current leaf.
    let user_turn = TurnLocation {
        workspace_id,
        session_id,
        parent_id: request.parent_id.or(session.current_message_id),
    };

    // Open the model turn BEFORE persisting anything: resolving the workspace's
    // language-model connection can fail (409 when none is configured), and a
    // failed send must not leave an orphan user turn in the history.
    let mut tokens = chat
        .stream_turn(&mut conn, user_turn, &request.content)
        .await?;

    // The turn resolved: persist the user message under the branch, advancing the
    // active leaf to it and seeding the title on the first message — all in one
    // transaction so the session state can't diverge from its messages.
    let user_message = chat
        .append_message(
            &mut conn,
            user_turn,
            ChatRole::User,
            &request.content,
            AppendSessionUpdate {
                advance_leaf: true,
                title: (session.title == DEFAULT_TITLE).then(|| seeded_title(&request.content)),
            },
        )
        .await?;

    // The assistant reply replies to the user message just stored.
    let reply_turn = TurnLocation {
        parent_id: Some(user_message.id),
        ..user_turn
    };

    drop(conn);

    let stream = stream! {
        let mut reply = String::new();
        // Only a normal end-of-stream (`None`) is a complete reply. A shutdown, a
        // generation error, or exceeding the reply limit stops mid-reply;
        // persisting that would store a partial turn as if the assistant had
        // finished, corrupting later history.
        let completed = loop {
            tokio::select! {
                // Server shutting down: end the open stream promptly so it does
                // not block graceful shutdown.
                () = shutdown.cancelled() => break false,
                next = tokens.next() => match next {
                    Some(Ok(delta)) => {
                        // Cap the reply so it always fits the encrypted-content
                        // column: a longer reply would fail to persist after the
                        // user already saw it, silently dropping it from history.
                        if reply.len() + delta.len() > MAX_REPLY_BYTES {
                            tracing::warn!(target: TRACING_TARGET, "Chat reply exceeded the size limit; stopping");
                            yield error_event("The response exceeded the maximum length and was stopped.");
                            break false;
                        }
                        reply.push_str(&delta);
                        yield token_event(&ChatToken { delta });
                    }
                    // A generation error: surface it and stop.
                    Some(Err(err)) => {
                        tracing::warn!(target: TRACING_TARGET, error = %err, "Chat generation failed");
                        yield error_event(&err.to_string());
                        break false;
                    }
                    // Generation finished normally.
                    None => break true,
                },
            }
        };

        // Persist the assembled reply only on normal completion (best-effort: the
        // user already saw it), under the user message it answered.
        if completed
            && !reply.is_empty()
            && let Err(err) = chat.persist_reply(reply_turn, &reply).await
        {
            tracing::error!(target: TRACING_TARGET, error = %err, "Failed to persist assistant reply");
        }
    };

    Ok(SseResponse::new(stream))
}

fn send_message_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Send chat message")
        .description(
            "Sends a message and streams the assistant's reply as Server-Sent \
             Events. Each event's `data` is a `ChatToken` delta. Authenticate \
             with a Bearer token via a `fetch`-based client; the native \
             `EventSource` cannot send an `Authorization` header. 409 when the \
             workspace has no language model connection configured.",
        )
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// One streamed chunk of the assistant's reply.
#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema
)]
#[serde(rename_all = "camelCase")]
pub struct ChatToken {
    /// The text delta.
    pub delta: String,
}

/// Builds a `token` SSE event carrying a reply delta.
fn token_event(token: &ChatToken) -> Event {
    Event::default()
        .event("token")
        .json_data(token)
        .unwrap_or_else(|_| Event::default().event("token"))
}

/// Builds an `error` SSE event carrying a failure message.
fn error_event(message: &str) -> Event {
    Event::default().event("error").data(message)
}

/// A session title seeded from the first message: trimmed to a single line and
/// capped so it reads well in a session list.
fn seeded_title(content: &str) -> String {
    let line = content.trim().lines().next().unwrap_or("").trim();
    let mut title: String = line.chars().take(TITLE_MAX).collect();
    if title.trim().is_empty() {
        title = "New chat".to_owned();
    }
    title
}

/// Returns the chat routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/chat/sessions/",
            post_with(create_session, create_session_docs)
                .get_with(list_sessions, list_sessions_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/chat/sessions/{sessionId}/",
            delete_with(delete_session, delete_session_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/chat/sessions/{sessionId}/messages/",
            get_with(list_messages, list_messages_docs).post_with(send_message, send_message_docs),
        )
        .with_path_items(|item| item.tag("Chat"))
}
