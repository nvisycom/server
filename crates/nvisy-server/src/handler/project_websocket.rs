//! WebSocket handler for real-time project communication.
//!
//! This module provides WebSocket endpoints for managing real-time communication
//! within a single project. It handles events such as:
//! - Document updates and collaborative editing
//! - Member presence tracking
//! - Project notifications
//! - Real-time synchronization of project state
//!
//! # Architecture
//!
//! Each WebSocket connection spawns two independent tasks:
//! - **Receiver task**: Handles incoming messages from the client
//! - **Sender task**: Subscribes to project broadcast channel and forwards messages
//!
//! The tasks run concurrently using `tokio::select!`, and if either task terminates,
//! the other is aborted to ensure clean shutdown.
//!
//! # Broadcasting
//!
//! Messages are broadcast to all connected clients in a project using a `tokio::sync::broadcast`
//! channel. Each project has its own channel stored in a shared state map.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use nvisy_postgres::PgDatabase;
use nvisy_postgres::queries::{AccountRepository, ProjectRepository};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{AuthState, Path};
use crate::handler::projects::ProjectPathParams;
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project websocket operations.
const TRACING_TARGET: &str = "nvisy::handler::project_websocket";

/// Maximum size of a WebSocket message in bytes (1 MB).
const MAX_MESSAGE_SIZE: usize = 1_024 * 1_024;

/// Capacity of broadcast channel per project (number of messages buffered).
const BROADCAST_CAPACITY: usize = 100;

/// WebSocket message types for project communication.
///
/// All messages are serialized as JSON with a `type` field that identifies
/// the message variant. This enables type-safe message handling on both
/// client and server.
///
/// Note: Protocol-level Ping/Pong is handled automatically by Axum.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProjectWsMessage {
    /// Client announces presence in the project.
    ///
    /// Sent automatically when a connection is established.
    #[serde(rename_all = "camelCase")]
    Join {
        account_id: Uuid,
        display_name: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Client leaves the project.
    ///
    /// Sent automatically when a connection is closed.
    #[serde(rename_all = "camelCase")]
    Leave {
        account_id: Uuid,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Document content update notification.
    #[serde(rename_all = "camelCase")]
    DocumentUpdate {
        document_id: Uuid,
        version: u32,
        updated_by: Uuid,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Document creation notification.
    #[serde(rename_all = "camelCase")]
    DocumentCreated {
        document_id: Uuid,
        display_name: String,
        created_by: Uuid,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Document deletion notification.
    #[serde(rename_all = "camelCase")]
    DocumentDeleted {
        document_id: Uuid,
        deleted_by: Uuid,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Member presence update.
    #[serde(rename_all = "camelCase")]
    MemberPresence {
        account_id: Uuid,
        is_online: bool,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Member added to project.
    #[serde(rename_all = "camelCase")]
    MemberAdded {
        account_id: Uuid,
        display_name: String,
        member_role: String,
        added_by: Uuid,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Member removed from project.
    #[serde(rename_all = "camelCase")]
    MemberRemoved {
        account_id: Uuid,
        removed_by: Uuid,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Project settings updated.
    #[serde(rename_all = "camelCase")]
    ProjectUpdated {
        display_name: Option<String>,
        updated_by: Uuid,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Typing indicator.
    ///
    /// Clients should throttle this message to avoid flooding the connection.
    #[serde(rename_all = "camelCase")]
    Typing {
        account_id: Uuid,
        document_id: Option<Uuid>,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: time::OffsetDateTime,
    },

    /// Error message from server.
    #[serde(rename_all = "camelCase")]
    Error {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
}

impl ProjectWsMessage {
    /// Creates an error message with the given code and message.
    #[inline]
    const fn error(code: String, message: String) -> Self {
        Self::Error {
            code,
            message,
            details: None,
        }
    }

    /// Creates an error message with additional details.
    #[inline]
    const fn error_with_details(code: String, message: String, details: String) -> Self {
        Self::Error {
            code,
            message,
            details: Some(details),
        }
    }

    /// Serializes the message to JSON text.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    fn to_text(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserializes a message from JSON text.
    ///
    /// # Errors
    ///
    /// Returns an error if the text is not valid JSON or doesn't match the schema.
    fn from_text(text: &str) -> std::result::Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

/// Context for a WebSocket connection.
#[derive(Debug, Clone)]
struct WsContext {
    /// Unique connection identifier for logging/debugging.
    connection_id: Uuid,
    /// The project this connection belongs to.
    project_id: Uuid,
    /// The authenticated account ID.
    account_id: Uuid,
    /// Display name of the account.
    display_name: String,
}

impl WsContext {
    /// Creates a new WebSocket connection context.
    fn new(project_id: Uuid, account_id: Uuid, display_name: String) -> Self {
        Self {
            connection_id: Uuid::new_v4(),
            project_id,
            account_id,
            display_name,
        }
    }
}

/// Global state for managing project broadcast channels.
///
/// Each project has its own broadcast channel for real-time communication.
/// Channels are created on-demand and cleaned up when no subscribers remain.
type ProjectChannels = Arc<RwLock<HashMap<Uuid, broadcast::Sender<ProjectWsMessage>>>>;

/// Gets or creates a broadcast channel for a project.
async fn get_project_channel(
    channels: &ProjectChannels,
    project_id: Uuid,
) -> broadcast::Sender<ProjectWsMessage> {
    // Try to get existing channel (read lock)
    {
        let read_guard = channels.read().await;
        if let Some(sender) = read_guard.get(&project_id) {
            return sender.clone();
        }
    }

    // Create new channel (write lock)
    let mut write_guard = channels.write().await;
    // Double-check in case another task created it
    if let Some(sender) = write_guard.get(&project_id) {
        return sender.clone();
    }

    let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
    write_guard.insert(project_id, sender.clone());

    tracing::debug!(
        target: "server::handler::project_websocket",
        project_id = %project_id,
        "created new broadcast channel for project"
    );

    sender
}

/// Processes an incoming WebSocket message.
///
/// Returns `ControlFlow::Break` if the connection should be closed,
/// `ControlFlow::Continue` otherwise.
fn process_message(
    ctx: &WsContext,
    msg: Message,
    tx: &broadcast::Sender<ProjectWsMessage>,
) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(text) => {
            // Check message size to prevent DoS
            if text.len() > MAX_MESSAGE_SIZE {
                tracing::warn!(
                    target: "server::handler::project_websocket",
                    connection_id = %ctx.connection_id,
                    message_size = text.len(),
                    max_size = MAX_MESSAGE_SIZE,
                    "message exceeds maximum size"
                );
                return ControlFlow::Continue(());
            }

            tracing::debug!(
                target: "server::handler::project_websocket",
                connection_id = %ctx.connection_id,
                message_length = text.len(),
                "received text message"
            );

            match ProjectWsMessage::from_text(&text) {
                Ok(ws_msg) => handle_project_message(ctx, ws_msg, tx),
                Err(e) => {
                    tracing::warn!(
                        target: "server::handler::project_websocket",
                        connection_id = %ctx.connection_id,
                        error = %e,
                        "failed to parse message"
                    );
                    ControlFlow::Continue(())
                }
            }
        }
        Message::Binary(data) => {
            if data.len() > MAX_MESSAGE_SIZE {
                tracing::warn!(
                    target: "server::handler::project_websocket",
                    connection_id = %ctx.connection_id,
                    data_length = data.len(),
                    max_size = MAX_MESSAGE_SIZE,
                    "binary message exceeds maximum size"
                );
                return ControlFlow::Continue(());
            }

            tracing::debug!(
                target: "server::handler::project_websocket",
                connection_id = %ctx.connection_id,
                data_length = data.len(),
                "received binary message"
            );

            // Binary messages can be used for compressed data or file chunks
            // For now, we just log them. Future: implement file transfer protocol
            ControlFlow::Continue(())
        }
        Message::Close(close_frame) => {
            if let Some(cf) = close_frame {
                tracing::info!(
                    target: "server::handler::project_websocket",
                    connection_id = %ctx.connection_id,
                    close_code = cf.code,
                    close_reason = %cf.reason,
                    "client sent close message"
                );
            } else {
                tracing::info!(
                    target: "server::handler::project_websocket",
                    connection_id = %ctx.connection_id,
                    "client sent close message without frame"
                );
            }
            ControlFlow::Break(())
        }
        // Axum automatically handles Ping/Pong at the protocol level.
        // These log messages are for observability only.
        Message::Ping(payload) => {
            tracing::trace!(
                target: "server::handler::project_websocket",
                connection_id = %ctx.connection_id,
                payload_len = payload.len(),
                "received protocol ping"
            );
            ControlFlow::Continue(())
        }
        Message::Pong(payload) => {
            tracing::trace!(
                target: "server::handler::project_websocket",
                connection_id = %ctx.connection_id,
                payload_len = payload.len(),
                "received protocol pong"
            );
            ControlFlow::Continue(())
        }
    }
}

/// Handles parsed project-specific messages and broadcasts them to other clients.
fn handle_project_message(
    ctx: &WsContext,
    msg: ProjectWsMessage,
    tx: &broadcast::Sender<ProjectWsMessage>,
) -> ControlFlow<(), ()> {
    match &msg {
        ProjectWsMessage::Typing { document_id, .. } => {
            tracing::trace!(
                target: "server::handler::project_websocket",
                connection_id = %ctx.connection_id,
                project_id = %ctx.project_id,
                document_id = ?document_id,
                "typing indicator received"
            );

            // Broadcast typing indicator to all other clients
            // Add timestamp to ensure freshness
            let msg_with_ts = ProjectWsMessage::Typing {
                account_id: ctx.account_id,
                document_id: *document_id,
                timestamp: time::OffsetDateTime::now_utc(),
            };

            if let Err(e) = tx.send(msg_with_ts) {
                tracing::debug!(
                    target: "server::handler::project_websocket",
                    connection_id = %ctx.connection_id,
                    error = %e,
                    "failed to broadcast typing indicator (no receivers)"
                );
            }
        }
        ProjectWsMessage::DocumentUpdate { .. }
        | ProjectWsMessage::DocumentCreated { .. }
        | ProjectWsMessage::DocumentDeleted { .. }
        | ProjectWsMessage::MemberPresence { .. }
        | ProjectWsMessage::MemberAdded { .. }
        | ProjectWsMessage::MemberRemoved { .. }
        | ProjectWsMessage::ProjectUpdated { .. } => {
            tracing::debug!(
                target: "server::handler::project_websocket",
                connection_id = %ctx.connection_id,
                message_type = ?std::mem::discriminant(&msg),
                "broadcasting message to project"
            );

            // Broadcast to all clients in the project
            if let Err(e) = tx.send(msg) {
                tracing::debug!(
                    target: "server::handler::project_websocket",
                    connection_id = %ctx.connection_id,
                    error = %e,
                    "failed to broadcast message (no receivers)"
                );
            }
        }
        _ => {
            tracing::debug!(
                target: "server::handler::project_websocket",
                connection_id = %ctx.connection_id,
                message_type = ?std::mem::discriminant(&msg),
                "received non-broadcast message"
            );
        }
    }
    ControlFlow::Continue(())
}

/// Handles the WebSocket connection lifecycle.
///
/// This function:
/// 1. Subscribes to the project's broadcast channel
/// 2. Sends a join message to all clients
/// 3. Spawns separate tasks for sending and receiving
/// 4. Uses `tokio::select!` to handle whichever task completes first
/// 5. Sends a leave message and aborts the other task
async fn handle_project_websocket(
    socket: WebSocket,
    project_id: Uuid,
    account_id: Uuid,
    display_name: String,
    channels: ProjectChannels,
) {
    let ctx = WsContext::new(project_id, account_id, display_name);

    tracing::info!(
        target: "server::handler::project_websocket",
        connection_id = %ctx.connection_id,
        account_id = %ctx.account_id,
        project_id = %ctx.project_id,
        "websocket connection established"
    );

    // Get or create broadcast channel for this project
    let tx = get_project_channel(&channels, project_id).await;
    let mut rx = tx.subscribe();

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Broadcast join message to all clients in the project
    let join_msg = ProjectWsMessage::Join {
        account_id: ctx.account_id,
        display_name: ctx.display_name.clone(),
        timestamp: time::OffsetDateTime::now_utc(),
    };

    if let Err(e) = tx.send(join_msg.clone()) {
        tracing::error!(
            target: TRACING_TARGET,
            connection_id = %ctx.connection_id,
            error = %e,
            "failed to broadcast join message"
        );
    }

    // Clone context for the receive task
    let recv_ctx = ctx.clone();
    let recv_tx = tx.clone();

    // Spawn a task to receive messages from the client
    let mut recv_task = tokio::spawn(async move {
        let mut msg_count = 0;
        while let Some(Ok(msg)) = receiver.next().await {
            msg_count += 1;
            if process_message(&recv_ctx, msg, &recv_tx).is_break() {
                break;
            }
        }
        msg_count
    });

    // Spawn a task to send messages to the client from the broadcast channel
    let send_ctx = ctx.clone();
    let mut send_task = tokio::spawn(async move {
        let mut msg_count = 0;

        // Send initial join message to this client
        if let Ok(text) = join_msg.to_text() {
            if sender
                .send(Message::Text(Utf8Bytes::from(text)))
                .await
                .is_err()
            {
                tracing::error!(
                    target: TRACING_TARGET,
                    connection_id = %send_ctx.connection_id,
                    "failed to send join message, aborting connection"
                );
                return 0;
            }
        }

        // Listen for broadcast messages and forward to this client
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    // Don't send messages back to the sender (echo prevention)
                    match &msg {
                        ProjectWsMessage::Typing { account_id, .. }
                        | ProjectWsMessage::Join { account_id, .. }
                        | ProjectWsMessage::Leave { account_id, .. }
                            if *account_id == send_ctx.account_id =>
                        {
                            continue;
                        }
                        _ => {}
                    }

                    if let Ok(text) = msg.to_text() {
                        if sender
                            .send(Message::Text(Utf8Bytes::from(text)))
                            .await
                            .is_err()
                        {
                            tracing::debug!(
                                target: TRACING_TARGET,
                                connection_id = %send_ctx.connection_id,
                                "failed to send message, client disconnected"
                            );
                            break;
                        }
                        msg_count += 1;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        connection_id = %send_ctx.connection_id,
                        skipped_messages = skipped,
                        "client lagged behind, some messages were dropped"
                    );
                    // Continue receiving - the client is slow but still connected
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        connection_id = %send_ctx.connection_id,
                        "broadcast channel closed"
                    );
                    break;
                }
            }
        }

        msg_count
    });

    // Wait for either task to complete, then abort the other
    tokio::select! {
        recv_result = (&mut recv_task) => {
            match recv_result {
                Ok(msg_count) => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        connection_id = %ctx.connection_id,
                        messages_received = msg_count,
                        "receive task completed"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        target: TRACING_TARGET,
                        connection_id = %ctx.connection_id,
                        error = %e,
                        "receive task panicked"
                    );
                }
            }
            send_task.abort();
        },
        send_result = (&mut send_task) => {
            match send_result {
                Ok(msg_count) => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        connection_id = %ctx.connection_id,
                        messages_sent = msg_count,
                        "send task completed"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        target: TRACING_TARGET,
                        connection_id = %ctx.connection_id,
                        error = %e,
                        "send task panicked"
                    );
                }
            }
            recv_task.abort();
        }
    }

    // Broadcast leave message
    let leave_msg = ProjectWsMessage::Leave {
        account_id: ctx.account_id,
        timestamp: time::OffsetDateTime::now_utc(),
    };
    let _ = tx.send(leave_msg);

    tracing::info!(
        target: TRACING_TARGET,
        connection_id = %ctx.connection_id,
        account_id = %ctx.account_id,
        project_id = %ctx.project_id,
        "websocket connection closed"
    );
}

/// Establishes a WebSocket connection for a project.
///
/// This endpoint upgrades an HTTP connection to a WebSocket for real-time
/// communication within a project. Clients must be authenticated and have
/// at least read access to the project.
///
/// # Security
///
/// - Requires valid JWT authentication
/// - Verifies user has `ReadAnyDocument` permission for the project
/// - Validates project existence before upgrading connection
/// - Enforces message size limits to prevent DoS attacks
///
/// # Connection Lifecycle
///
/// 1. Client sends upgrade request with Authorization header
/// 2. Server validates authentication and project access
/// 3. Connection is upgraded to WebSocket
/// 4. Server broadcasts `Join` message to all project clients
/// 5. Two concurrent tasks handle sending and receiving
/// 6. Messages are broadcast to all connected clients via channel
/// 7. Server broadcasts `Leave` message on disconnect
/// 8. Connection closed gracefully
#[tracing::instrument(skip_all, fields(
    account_id = %auth_claims.account_id,
    project_id = %path_params.project_id
))]
#[utoipa::path(
    get, path = "/projects/{projectId}/ws/", tag = "projects",
    params(ProjectPathParams),
    responses(
        (
            status = SWITCHING_PROTOCOLS,
            description = "WebSocket connection established",
        ),
        (
            status = UNAUTHORIZED,
            description = "Authentication required",
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
        ),
        (
            status = NOT_FOUND,
            description = "Project not found",
        ),
    ),
)]
async fn project_websocket_handler(
    State(pg_database): State<PgDatabase>,
    State(channels): State<ProjectChannels>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ws: WebSocketUpgrade,
) -> Result<Response> {
    let project_id = path_params.project_id;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        project_id = %project_id,
        "websocket connection requested"
    );

    // Verify project exists and user has access
    let mut conn = pg_database.get_connection().await?;

    // Check if user has permission to access this project
    AuthService::authorize_project(
        &mut conn,
        &auth_claims,
        project_id,
        ProjectPermission::ReadAnyDocument,
    )
    .await?;

    // Verify the project exists
    ProjectRepository::find_project_by_id(&mut conn, project_id)
        .await?
        .ok_or_else(|| ErrorKind::NotFound.with_resource("project"))?;

    // Fetch account display name
    let account = AccountRepository::find_account_by_id(&mut conn, auth_claims.account_id)
        .await?
        .ok_or_else(|| ErrorKind::NotFound.with_resource("account"))?;

    let display_name = account.display_name;
    let account_id = auth_claims.account_id;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account_id,
        project_id = %project_id,
        "websocket upgrade authorized"
    );

    // Upgrade the connection to WebSocket
    Ok(ws.on_upgrade(move |socket| {
        handle_project_websocket(socket, project_id, account_id, display_name, channels)
    }))
}

/// Returns a [`Router`] with WebSocket routes for projects.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(project_websocket_handler))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_message_serialization_join() {
        let msg = ProjectWsMessage::Join {
            account_id: Uuid::new_v4(),
            display_name: "Test User".to_string(),
            timestamp: time::OffsetDateTime::now_utc(),
        };

        let json = msg.to_text().unwrap();
        let parsed = ProjectWsMessage::from_text(&json).unwrap();

        match parsed {
            ProjectWsMessage::Join { display_name, .. } => {
                assert_eq!(display_name, "Test User");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_message_serialization_leave() {
        let account_id = Uuid::new_v4();
        let msg = ProjectWsMessage::Leave {
            account_id,
            timestamp: time::OffsetDateTime::now_utc(),
        };

        let json = msg.to_text().unwrap();
        let parsed = ProjectWsMessage::from_text(&json).unwrap();

        match parsed {
            ProjectWsMessage::Leave {
                account_id: parsed_id,
                ..
            } => {
                assert_eq!(parsed_id, account_id);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_typing_with_timestamp() {
        let msg = ProjectWsMessage::Typing {
            account_id: Uuid::new_v4(),
            document_id: Some(Uuid::new_v4()),
            timestamp: time::OffsetDateTime::now_utc(),
        };

        let json = msg.to_text().unwrap();
        assert!(json.contains("\"type\":\"typing\""));
        assert!(json.contains("timestamp"));

        let parsed = ProjectWsMessage::from_text(&json).unwrap();
        match parsed {
            ProjectWsMessage::Typing { .. } => {}
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_error_message_with_details() {
        let error = ProjectWsMessage::error_with_details(
            "PARSE_ERROR".to_string(),
            "Invalid JSON".to_string(),
            "Expected } at line 5".to_string(),
        );
        let json = error.to_text().unwrap();
        assert!(json.contains("PARSE_ERROR"));
        assert!(json.contains("Invalid JSON"));
        assert!(json.contains("Expected } at line 5"));
    }

    #[test]
    fn test_document_update_serialization() {
        let msg = ProjectWsMessage::DocumentUpdate {
            document_id: Uuid::new_v4(),
            version: 42,
            updated_by: Uuid::new_v4(),
            timestamp: time::OffsetDateTime::now_utc(),
        };

        let json = msg.to_text().unwrap();
        let parsed = ProjectWsMessage::from_text(&json).unwrap();

        match parsed {
            ProjectWsMessage::DocumentUpdate { version, .. } => {
                assert_eq!(version, 42);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
