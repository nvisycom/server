//! WebSocket handler for real-time workspace communication via NATS.

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use nvisy_nats::NatsClient;
use nvisy_nats::stream::{WorkspaceEventPublisher, WorkspaceWsMessage};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::{AccountRepository, WorkspaceRepository};
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission};
use crate::handler::request::WorkspacePathParams;
use crate::handler::response::ErrorResponse;
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for workspace websocket operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_websocket";

/// Maximum size of a WebSocket message in bytes (1 MB).
const MAX_MESSAGE_SIZE: usize = 1_024 * 1_024;

/// Timeout for fetching messages from NATS stream.
const NATS_FETCH_TIMEOUT: Duration = Duration::from_millis(100);

/// Maximum time to wait for graceful connection shutdown.
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Context for a WebSocket connection.
#[derive(Debug, Clone)]
struct WsContext {
    /// Unique connection identifier for logging/debugging.
    connection_id: Uuid,
    /// The workspace this connection belongs to.
    workspace_id: Uuid,
    /// The authenticated account ID.
    account_id: Uuid,
}

impl WsContext {
    /// Creates a new WebSocket connection context.
    fn new(workspace_id: Uuid, account_id: Uuid) -> Self {
        Self {
            connection_id: Uuid::new_v4(),
            workspace_id,
            account_id,
        }
    }
}

/// Metrics for a WebSocket connection.
#[derive(Debug, Default)]
struct ConnectionMetrics {
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    messages_published: AtomicU64,
    messages_dropped: AtomicU64,
    errors: AtomicU64,
}

impl ConnectionMetrics {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn increment_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_received(&self) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_published(&self) {
        self.messages_published.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_dropped(&self) {
        self.messages_dropped.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_errors(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            sent: self.messages_sent.load(Ordering::Relaxed),
            received: self.messages_received.load(Ordering::Relaxed),
            published: self.messages_published.load(Ordering::Relaxed),
            dropped: self.messages_dropped.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
struct MetricsSnapshot {
    sent: u64,
    received: u64,
    published: u64,
    dropped: u64,
    errors: u64,
}

/// Validate message size to prevent DoS attacks.
fn validate_message_size(ctx: &WsContext, size: usize, metrics: &ConnectionMetrics) -> bool {
    if size > MAX_MESSAGE_SIZE {
        tracing::warn!(
            target: TRACING_TARGET,
            connection_id = %ctx.connection_id,
            message_size = size,
            max_size = MAX_MESSAGE_SIZE,
            "message exceeds maximum size, dropping"
        );
        metrics.increment_dropped();
        false
    } else {
        true
    }
}

/// Check if the account has permission to perform the action in the message.
async fn check_event_permission(
    conn: &mut nvisy_postgres::PgConn,
    ctx: &WsContext,
    msg: &WorkspaceWsMessage,
) -> Result<()> {
    // Determine required permission based on message type
    let permission = match msg {
        // Read-only events - require ViewDocuments permission
        WorkspaceWsMessage::Typing(_) | WorkspaceWsMessage::MemberPresence(_) => {
            Permission::ViewDocuments
        }

        // Document write events - require UpdateDocuments permission
        WorkspaceWsMessage::DocumentUpdate(_) => Permission::UpdateDocuments,
        WorkspaceWsMessage::DocumentCreated(_) => Permission::CreateDocuments,
        WorkspaceWsMessage::DocumentDeleted(_) => Permission::DeleteDocuments,

        // File events - require appropriate file permissions
        WorkspaceWsMessage::FilePreprocessed(_)
        | WorkspaceWsMessage::FilePostprocessed(_)
        | WorkspaceWsMessage::JobProgress(_)
        | WorkspaceWsMessage::JobCompleted(_)
        | WorkspaceWsMessage::JobFailed(_) => Permission::ViewFiles,
        WorkspaceWsMessage::FileTransformed(_) => Permission::UpdateFiles,

        // Member management - require InviteMembers/RemoveMembers permission
        WorkspaceWsMessage::MemberAdded(_) => Permission::InviteMembers,
        WorkspaceWsMessage::MemberRemoved(_) => Permission::RemoveMembers,

        // Workspace settings - require UpdateWorkspace permission
        WorkspaceWsMessage::WorkspaceUpdated(_) => Permission::UpdateWorkspace,

        // System events - always allowed (sent by server)
        WorkspaceWsMessage::Join(_)
        | WorkspaceWsMessage::Leave(_)
        | WorkspaceWsMessage::Error(_) => {
            return Ok(());
        }
    };

    // Fetch workspace membership directly
    use nvisy_postgres::query::WorkspaceMemberRepository;

    let member = conn
        .find_workspace_member(ctx.workspace_id, ctx.account_id)
        .await?;

    // Check if member exists and has the required permission
    match member {
        Some(m) if permission.is_permitted_by_role(m.member_role) => Ok(()),
        Some(m) => {
            tracing::debug!(
                target: TRACING_TARGET,
                account_id = %ctx.account_id,
                workspace_id = %ctx.workspace_id,
                required_permission = ?permission,
                current_role = ?m.member_role,
                "insufficient permissions for event"
            );
            Err(ErrorKind::Forbidden.with_context(format!(
                "Insufficient permissions: requires {:?}",
                permission.minimum_required_role()
            )))
        }
        None => {
            tracing::debug!(
                target: TRACING_TARGET,
                account_id = %ctx.account_id,
                workspace_id = %ctx.workspace_id,
                "not a member of workspace"
            );
            Err(ErrorKind::Forbidden.with_context("Not a workspace member"))
        }
    }
}

/// Processes an incoming WebSocket message from the client.
async fn process_client_message(
    ctx: &WsContext,
    msg: Message,
    publisher: &WorkspaceEventPublisher,
    conn: &mut nvisy_postgres::PgConn,
    metrics: &ConnectionMetrics,
) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(text) => {
            metrics.increment_received();

            if !validate_message_size(ctx, text.len(), metrics) {
                return ControlFlow::Continue(());
            }

            tracing::trace!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                message_length = text.len(),
                "received text message"
            );

            match serde_json::from_str::<WorkspaceWsMessage>(&text) {
                Ok(ws_msg) => {
                    handle_client_message(ctx, ws_msg, publisher, conn, metrics).await;
                    ControlFlow::Continue(())
                }
                Err(e) => {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        connection_id = %ctx.connection_id,
                        error = %e,
                        "failed to parse message, dropping"
                    );
                    metrics.increment_errors();
                    metrics.increment_dropped();
                    ControlFlow::Continue(())
                }
            }
        }
        Message::Binary(data) => {
            metrics.increment_received();

            if !validate_message_size(ctx, data.len(), metrics) {
                return ControlFlow::Continue(());
            }

            tracing::debug!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                data_length = data.len(),
                "received binary message (not supported), dropping"
            );
            metrics.increment_dropped();
            ControlFlow::Continue(())
        }
        Message::Close(close_frame) => {
            if let Some(cf) = close_frame {
                tracing::info!(
                    target: TRACING_TARGET,
                    connection_id = %ctx.connection_id,
                    close_code = cf.code,
                    close_reason = %cf.reason,
                    "client sent close frame"
                );
            } else {
                tracing::info!(
                    target: TRACING_TARGET,
                    connection_id = %ctx.connection_id,
                    "client sent close frame"
                );
            }
            ControlFlow::Break(())
        }
        Message::Ping(payload) => {
            tracing::trace!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                payload_len = payload.len(),
                "received ping"
            );
            ControlFlow::Continue(())
        }
        Message::Pong(payload) => {
            tracing::trace!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                payload_len = payload.len(),
                "received pong"
            );
            ControlFlow::Continue(())
        }
    }
}

/// Handles parsed messages from the client with permission checking.
async fn handle_client_message(
    ctx: &WsContext,
    msg: WorkspaceWsMessage,
    publisher: &WorkspaceEventPublisher,
    conn: &mut nvisy_postgres::PgConn,
    metrics: &ConnectionMetrics,
) {
    // Check permissions for this event
    if let Err(e) = check_event_permission(conn, ctx, &msg).await {
        tracing::warn!(
            target: TRACING_TARGET,
            connection_id = %ctx.connection_id,
            account_id = %ctx.account_id,
            message_type = ?std::mem::discriminant(&msg),
            error = %e,
            "permission denied for event, dropping"
        );
        metrics.increment_dropped();
        metrics.increment_errors();
        return;
    }

    match &msg {
        WorkspaceWsMessage::Typing(_) => {
            tracing::trace!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                "publishing typing indicator"
            );

            // Publish with fresh timestamp
            let msg_with_ts = WorkspaceWsMessage::typing(ctx.account_id, None);

            if let Err(e) = publisher
                .publish_message(ctx.workspace_id, msg_with_ts)
                .await
            {
                tracing::warn!(
                    target: TRACING_TARGET,
                    connection_id = %ctx.connection_id,
                    error = %e,
                    "failed to publish typing indicator"
                );
                metrics.increment_errors();
            } else {
                metrics.increment_published();
            }
        }
        WorkspaceWsMessage::DocumentUpdate(_)
        | WorkspaceWsMessage::DocumentCreated(_)
        | WorkspaceWsMessage::DocumentDeleted(_)
        | WorkspaceWsMessage::FilePreprocessed(_)
        | WorkspaceWsMessage::FileTransformed(_)
        | WorkspaceWsMessage::FilePostprocessed(_)
        | WorkspaceWsMessage::JobProgress(_)
        | WorkspaceWsMessage::JobCompleted(_)
        | WorkspaceWsMessage::JobFailed(_)
        | WorkspaceWsMessage::MemberPresence(_)
        | WorkspaceWsMessage::MemberAdded(_)
        | WorkspaceWsMessage::MemberRemoved(_)
        | WorkspaceWsMessage::WorkspaceUpdated(_) => {
            tracing::debug!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                message_type = ?std::mem::discriminant(&msg),
                "publishing event to NATS"
            );

            if let Err(e) = publisher.publish_message(ctx.workspace_id, msg).await {
                tracing::warn!(
                    target: TRACING_TARGET,
                    connection_id = %ctx.connection_id,
                    error = %e,
                    "failed to publish event to NATS"
                );
                metrics.increment_errors();
            } else {
                metrics.increment_published();
            }
        }
        WorkspaceWsMessage::Join(_)
        | WorkspaceWsMessage::Leave(_)
        | WorkspaceWsMessage::Error(_) => {
            tracing::debug!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                message_type = ?std::mem::discriminant(&msg),
                "ignoring system message from client"
            );
            metrics.increment_dropped();
        }
    }
}

/// Handles the WebSocket connection lifecycle with NATS pub/sub.
///
/// This function:
/// 1. Fetches account details and creates context
/// 2. Creates a unique NATS consumer for this WebSocket connection
/// 3. Publishes a join message to all clients
/// 4. Spawns separate tasks for sending and receiving
/// 5. Uses `tokio::select!` to handle whichever task completes first
/// 6. Publishes a leave message and cleans up
async fn handle_workspace_websocket(
    socket: WebSocket,
    workspace_id: Uuid,
    account_id: Uuid,
    nats_client: NatsClient,
    pg_client: PgClient,
) {
    let start_time = std::time::Instant::now();
    let ctx = WsContext::new(workspace_id, account_id);
    let metrics = ConnectionMetrics::new();

    tracing::info!(
        target: TRACING_TARGET,
        connection_id = %ctx.connection_id,
        account_id = %ctx.account_id,
        workspace_id = %ctx.workspace_id,
        "websocket connection established"
    );

    // Get a connection for initial queries
    let mut conn = match pg_client.get_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                error = %e,
                "failed to acquire database connection"
            );
            return;
        }
    };

    // Fetch account display name
    let display_name = match conn.find_account_by_id(account_id).await {
        Ok(Some(account)) => account.display_name,
        Ok(None) => {
            tracing::error!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                account_id = %account_id,
                "account not found, aborting connection"
            );
            return;
        }
        Err(e) => {
            tracing::error!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                account_id = %account_id,
                error = %e,
                "failed to fetch account, aborting connection"
            );
            return;
        }
    };

    // Create publisher for this connection
    let publisher = match nats_client.workspace_event_publisher().await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                error = %e,
                "failed to create event publisher, aborting connection"
            );
            return;
        }
    };

    // Create subscriber with unique consumer name for this connection
    let consumer_name = format!("ws-{}", ctx.connection_id);
    let subscriber = match nats_client
        .workspace_event_subscriber_for_workspace(&consumer_name, workspace_id)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                error = %e,
                "failed to create event subscriber, aborting connection"
            );
            return;
        }
    };

    // Get message stream
    let mut message_stream = match subscriber.subscribe().await {
        Ok(stream) => stream,
        Err(e) => {
            tracing::error!(
                target: TRACING_TARGET,
                connection_id = %ctx.connection_id,
                error = %e,
                "failed to subscribe to event stream, aborting connection"
            );
            return;
        }
    };

    tracing::debug!(
        target: TRACING_TARGET,
        connection_id = %ctx.connection_id,
        consumer_name = %consumer_name,
        "NATS subscriber created"
    );

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Create and publish join message
    let join_msg = WorkspaceWsMessage::join(ctx.account_id, display_name);

    if let Err(e) = publisher
        .publish_message(ctx.workspace_id, join_msg.clone())
        .await
    {
        tracing::error!(
            target: TRACING_TARGET,
            connection_id = %ctx.connection_id,
            error = %e,
            "failed to publish join message"
        );
    } else {
        metrics.increment_published();
    }

    // Clone context and clients for the receive task
    let recv_ctx = ctx.clone();
    let recv_publisher = publisher.clone();
    let recv_pg_client = pg_client.clone();
    let recv_metrics = metrics.clone();

    // Spawn a task to receive messages from the client
    let recv_task = tokio::spawn(async move {
        // Get a dedicated connection for the receive task
        let mut recv_conn = match recv_pg_client.get_connection().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    connection_id = %recv_ctx.connection_id,
                    error = %e,
                    "failed to acquire database connection for receive task"
                );
                return;
            }
        };

        while let Some(msg_result) = receiver.next().await {
            match msg_result {
                Ok(msg) => {
                    if process_client_message(
                        &recv_ctx,
                        msg,
                        &recv_publisher,
                        &mut recv_conn,
                        &recv_metrics,
                    )
                    .await
                    .is_break()
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        connection_id = %recv_ctx.connection_id,
                        error = %e,
                        "error receiving from websocket"
                    );
                    recv_metrics.increment_errors();
                    break;
                }
            }
        }
    });

    // Spawn a task to send messages from NATS to the client
    let send_ctx = ctx.clone();
    let send_metrics = metrics.clone();
    let send_task = tokio::spawn(async move {
        // Send initial join message to this client
        if let Ok(text) = serde_json::to_string(&join_msg) {
            if let Err(e) = sender.send(Message::Text(Utf8Bytes::from(text))).await {
                tracing::error!(
                    target: TRACING_TARGET,
                    connection_id = %send_ctx.connection_id,
                    error = %e,
                    "failed to send join message, aborting connection"
                );
                return;
            }
            send_metrics.increment_sent();
        }

        // Listen for NATS messages and forward to this client
        loop {
            match message_stream.next_with_timeout(NATS_FETCH_TIMEOUT).await {
                Ok(Some(mut nats_msg)) => {
                    let ws_message = &nats_msg.payload().message;

                    // Echo prevention: don't send messages back to the sender
                    if let Some(sender_id) = ws_message.account_id()
                        && sender_id == send_ctx.account_id
                    {
                        if let Err(e) = nats_msg.ack().await {
                            tracing::trace!(
                                target: TRACING_TARGET,
                                connection_id = %send_ctx.connection_id,
                                error = %e,
                                "failed to ack echoed message"
                            );
                        }
                        continue;
                    }

                    // Serialize and send the message
                    match serde_json::to_string(ws_message) {
                        Ok(text) => {
                            if let Err(e) = sender.send(Message::Text(Utf8Bytes::from(text))).await
                            {
                                tracing::debug!(
                                    target: TRACING_TARGET,
                                    connection_id = %send_ctx.connection_id,
                                    error = %e,
                                    "failed to send message, client disconnected"
                                );
                                break;
                            }
                            send_metrics.increment_sent();

                            // Acknowledge the message
                            if let Err(e) = nats_msg.ack().await {
                                tracing::warn!(
                                    target: TRACING_TARGET,
                                    connection_id = %send_ctx.connection_id,
                                    error = %e,
                                    "failed to acknowledge NATS message"
                                );
                                send_metrics.increment_errors();
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                target: TRACING_TARGET,
                                connection_id = %send_ctx.connection_id,
                                error = %e,
                                "failed to serialize message"
                            );
                            send_metrics.increment_errors();

                            // Still ack to prevent redelivery
                            let _ = nats_msg.ack().await;
                        }
                    }
                }
                Ok(None) => {
                    // Timeout - continue waiting
                    continue;
                }
                Err(e) => {
                    tracing::error!(
                        target: TRACING_TARGET,
                        connection_id = %send_ctx.connection_id,
                        error = %e,
                        "error receiving from NATS stream"
                    );
                    send_metrics.increment_errors();
                    break;
                }
            }
        }
    });

    // Wait for either task to complete with graceful shutdown
    let shutdown_result = tokio::time::timeout(GRACEFUL_SHUTDOWN_TIMEOUT, async {
        tokio::select! {
            _ = recv_task => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    connection_id = %ctx.connection_id,
                    "receive task completed"
                );
            },
            _ = send_task => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    connection_id = %ctx.connection_id,
                    "send task completed"
                );
            }
        }
    })
    .await;

    if shutdown_result.is_err() {
        tracing::warn!(
            target: TRACING_TARGET,
            connection_id = %ctx.connection_id,
            "graceful shutdown timeout exceeded"
        );
    }

    // Publish leave message
    let leave_msg = WorkspaceWsMessage::leave(ctx.account_id);
    if let Err(e) = publisher.publish_message(ctx.workspace_id, leave_msg).await {
        tracing::warn!(
            target: TRACING_TARGET,
            connection_id = %ctx.connection_id,
            error = %e,
            "failed to publish leave message"
        );
    }

    // Log final metrics
    let duration = start_time.elapsed();
    let final_metrics = metrics.snapshot();
    tracing::info!(
        target: TRACING_TARGET,
        connection_id = %ctx.connection_id,
        account_id = %ctx.account_id,
        workspace_id = %ctx.workspace_id,
        duration_ms = duration.as_millis(),
        messages_sent = final_metrics.sent,
        messages_received = final_metrics.received,
        messages_published = final_metrics.published,
        messages_dropped = final_metrics.dropped,
        errors = final_metrics.errors,
        "websocket connection closed"
    );
}

/// Establishes a WebSocket connection for a workspace.
#[tracing::instrument(skip_all, fields(
    account_id = %auth_claims.account_id,
    workspace_id = %path_params.workspace_id
))]
async fn workspace_websocket_handler(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ws: WebSocketUpgrade,
) -> Result<Response> {
    let workspace_id = path_params.workspace_id;
    let account_id = auth_claims.account_id;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = %account_id,
        workspace_id = %workspace_id,
        "websocket connection requested"
    );

    // Verify workspace exists and user has basic access
    let mut conn = pg_client.get_connection().await?;

    // Check if user has minimum permission to view documents
    auth_claims
        .authorize_workspace(&mut conn, workspace_id, Permission::ViewDocuments)
        .await?;

    // Verify the workspace exists
    if conn.find_workspace_by_id(workspace_id).await?.is_none() {
        return Err(ErrorKind::NotFound.with_resource("workspace"));
    }

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account_id,
        workspace_id = %workspace_id,
        "websocket upgrade authorized"
    );

    // Upgrade the connection to WebSocket
    Ok(ws.on_upgrade(move |socket| {
        handle_workspace_websocket(socket, workspace_id, account_id, nats_client, pg_client)
    }))
}

fn workspace_websocket_handler_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Connect to workspace WebSocket")
        .description(
            "Establishes a WebSocket connection for real-time workspace events and collaboration.",
        )
        .response::<101, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with WebSocket routes for workspaces.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/ws/",
            get_with(
                workspace_websocket_handler,
                workspace_websocket_handler_docs,
            ),
        )
        .with_path_items(|item| item.tag("WebSocket"))
}
