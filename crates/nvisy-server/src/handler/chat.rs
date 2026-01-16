//! Chat session handlers for LLM-assisted document editing.
//!
//! This module provides comprehensive chat session management functionality within workspaces,
//! including creation, reading, updating, and deletion of sessions. All operations
//! are secured with proper authorization and follow workspace-based access control.
//!
//! ## Streaming
//!
//! The `/chat/sessions/{sessionId}/messages` endpoint uses Server-Sent Events (SSE) to stream
//! LLM responses back to the client. Clients can cancel generation by closing the connection
//! (e.g., using `AbortController` in JavaScript).

use std::convert::Infallible;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::StreamExt;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::ChatSessionRepository;
use nvisy_rig::RigService;
use tokio_stream::wrappers::ReceiverStream;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    ChatSessionPathParams, CreateChatSession, CursorPagination, SendChatMessage, UpdateChatSession,
    WorkspacePathParams,
};
use crate::handler::response::{ChatSession, ChatSessionsPage, ChatStreamEvent, ErrorResponse};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for chat session operations.
const TRACING_TARGET: &str = "nvisy_server::handler::chat";

/// Creates a new chat session.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn create_chat_session(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<CreateChatSession>,
) -> Result<(StatusCode, Json<ChatSession>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating chat session");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::CreateDocuments,
        )
        .await?;

    let new_session = request.into_model(path_params.workspace_id, auth_state.account_id);
    let session = conn.create_chat_session(new_session).await?;

    tracing::info!(
        target: TRACING_TARGET,
        session_id = %session.id,
        "Chat session created",
    );

    Ok((StatusCode::CREATED, Json(ChatSession::from_model(session))))
}

fn create_chat_session_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create chat session")
        .description("Creates a new LLM-assisted editing session for a document file.")
        .response::<201, Json<ChatSession>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Returns all chat sessions for a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn get_all_chat_sessions(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<ChatSessionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing chat sessions");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewDocuments,
        )
        .await?;

    let page = conn
        .cursor_list_chat_sessions(path_params.workspace_id, pagination.into())
        .await?;

    let response = ChatSessionsPage::from_cursor_page(page, ChatSession::from_model);

    tracing::debug!(
        target: TRACING_TARGET,
        session_count = response.items.len(),
        "Chat sessions listed",
    );

    Ok((StatusCode::OK, Json(response)))
}

fn get_all_chat_sessions_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List chat sessions")
        .description("Lists all chat sessions in a workspace with pagination.")
        .response::<200, Json<ChatSessionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Gets a chat session by its session ID.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        session_id = %path_params.session_id,
    )
)]
async fn get_chat_session(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ChatSessionPathParams>,
) -> Result<(StatusCode, Json<ChatSession>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading chat session");

    let mut conn = pg_client.get_connection().await?;

    let session = find_chat_session(&mut conn, path_params.session_id).await?;

    auth_state
        .authorize_workspace(&mut conn, session.workspace_id, Permission::ViewDocuments)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Chat session read");

    Ok((StatusCode::OK, Json(ChatSession::from_model(session))))
}

fn get_chat_session_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get chat session")
        .description("Returns chat session details by ID.")
        .response::<200, Json<ChatSession>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a chat session by its session ID.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        session_id = %path_params.session_id,
    )
)]
async fn update_chat_session(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ChatSessionPathParams>,
    ValidateJson(request): ValidateJson<UpdateChatSession>,
) -> Result<(StatusCode, Json<ChatSession>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating chat session");

    let mut conn = pg_client.get_connection().await?;

    let existing = find_chat_session(&mut conn, path_params.session_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            existing.workspace_id,
            Permission::UpdateDocuments,
        )
        .await?;

    let update_data = request.into_model();
    let session = conn
        .update_chat_session(path_params.session_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Chat session updated");

    Ok((StatusCode::OK, Json(ChatSession::from_model(session))))
}

fn update_chat_session_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update chat session")
        .description("Updates chat session metadata and configuration.")
        .response::<200, Json<ChatSession>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes (archives) a chat session by its session ID.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        session_id = %path_params.session_id,
    )
)]
async fn delete_chat_session(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ChatSessionPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting chat session");

    let mut conn = pg_client.get_connection().await?;

    let session = find_chat_session(&mut conn, path_params.session_id).await?;

    auth_state
        .authorize_workspace(&mut conn, session.workspace_id, Permission::DeleteDocuments)
        .await?;

    conn.delete_chat_session(path_params.session_id).await?;

    tracing::info!(target: TRACING_TARGET, "Chat session deleted");

    Ok(StatusCode::OK)
}

fn delete_chat_session_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete chat session")
        .description("Archives the chat session (soft delete).")
        .response_with::<200, (), _>(|res| res.description("Chat session deleted."))
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Sends a message to a chat session and streams the response via SSE.
///
/// The response is streamed as Server-Sent Events with different event types:
/// - `thinking`: Agent is processing/planning
/// - `text_delta`: Incremental text from the LLM
/// - `tool_call`: Agent is calling a tool
/// - `tool_result`: Tool execution completed
/// - `proposed_edit`: Agent proposes a document edit
/// - `edit_applied`: Edit was auto-applied
/// - `done`: Response completed with final summary
/// - `error`: An error occurred
///
/// Clients can cancel generation by closing the connection (AbortController).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        session_id = %path_params.session_id,
    )
)]
async fn send_message(
    State(pg_client): State<PgClient>,
    State(rig_service): State<RigService>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ChatSessionPathParams>,
    ValidateJson(request): ValidateJson<SendChatMessage>,
) -> Result<impl axum::response::IntoResponse> {
    tracing::debug!(target: TRACING_TARGET, "Sending chat message");

    let mut conn = pg_client.get_connection().await?;

    // Verify session exists and user has access
    let session = find_chat_session(&mut conn, path_params.session_id).await?;

    auth_state
        .authorize_workspace(&mut conn, session.workspace_id, Permission::UpdateDocuments)
        .await?;

    // Create SSE stream
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    // Get the chat stream from rig service
    let chat_stream = rig_service
        .chat()
        .chat(path_params.session_id, &request.content)
        .await
        .map_err(|e| {
            tracing::error!(target: TRACING_TARGET, error = %e, "Failed to create chat stream");
            ErrorKind::InternalServerError
                .with_message("Failed to start chat")
                .with_context(e.to_string())
        })?;

    // Spawn task to process the chat stream and send SSE events
    let session_id = path_params.session_id;
    tokio::spawn(async move {
        let mut stream = std::pin::pin!(chat_stream);

        while let Some(result) = stream.next().await {
            let event = match result {
                Ok(chat_event) => {
                    let stream_event = ChatStreamEvent::new(chat_event);
                    let event_type = stream_event.event_type();

                    match serde_json::to_string(&stream_event) {
                        Ok(json) => Event::default().event(event_type).data(json),
                        Err(e) => {
                            tracing::error!(
                                target: TRACING_TARGET,
                                session_id = %session_id,
                                error = %e,
                                "Failed to serialize chat event"
                            );
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        target: TRACING_TARGET,
                        session_id = %session_id,
                        error = %e,
                        "Chat stream error"
                    );
                    // Send error event and break
                    let error_event = ChatStreamEvent::new(nvisy_rig::chat::ChatEvent::Error {
                        message: e.to_string(),
                    });
                    if let Ok(json) = serde_json::to_string(&error_event) {
                        let _ = tx
                            .send(Ok(Event::default().event("error").data(json)))
                            .await;
                    }
                    break;
                }
            };

            // Send the event; if send fails, client disconnected (cancelled)
            if tx.send(Ok(event)).await.is_err() {
                tracing::info!(
                    target: TRACING_TARGET,
                    session_id = %session_id,
                    "Client disconnected, cancelling chat stream"
                );
                break;
            }
        }

        tracing::debug!(
            target: TRACING_TARGET,
            session_id = %session_id,
            "Chat stream completed"
        );
    });

    tracing::info!(
        target: TRACING_TARGET,
        session_id = %path_params.session_id,
        "Chat message stream started"
    );

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

/// Finds a chat session by ID or returns NotFound error.
async fn find_chat_session(
    conn: &mut nvisy_postgres::PgConn,
    session_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::ChatSession> {
    conn.find_chat_session_by_id(session_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Chat session not found.")
                .with_resource("chat_session")
        })
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/chat/sessions",
            post_with(create_chat_session, create_chat_session_docs)
                .get_with(get_all_chat_sessions, get_all_chat_sessions_docs),
        )
        .api_route(
            "/chat/sessions/{sessionId}",
            get_with(get_chat_session, get_chat_session_docs)
                .patch_with(update_chat_session, update_chat_session_docs)
                .delete_with(delete_chat_session, delete_chat_session_docs),
        )
        // SSE endpoint - uses regular axum routing as aide doesn't support SSE in OpenAPI
        .route(
            "/chat/sessions/{sessionId}/messages",
            axum::routing::post(send_message),
        )
        .with_path_items(|item| item.tag("Chat"))
}
