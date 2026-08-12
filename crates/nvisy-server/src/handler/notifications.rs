//! Notification handlers for account notification operations.
//!
//! This module provides handlers for listing notifications, checking unread
//! status, and marking notifications as read for the authenticated account.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::AccountNotificationRepository;

use crate::extract::{AuthState, Json, Path, Query};
use crate::handler::request::{CursorPagination, NotificationPathParams};
use crate::handler::response::{
    ErrorResponse, MarkedReadStatus, Notification, NotificationsPage, UnreadStatus,
};
use crate::handler::{Error, Result};
use crate::service::ServiceState;

/// Tracing target for notification operations.
const TRACING_TARGET: &str = "nvisy_server::handler::notifications";

/// Lists notifications for the authenticated account.
///
/// A pure read: it does not change read status. The client marks notifications
/// read explicitly via `POST /notifications/read/` (all) or
/// `POST /notifications/{notificationId}/read/` (one).
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_state.account_id)
)]
async fn list_notifications(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<NotificationsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing notifications");

    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_account_notifications(auth_state.account_id, pagination.into())
        .await?;

    let response = NotificationsPage::from_cursor_page(page, Notification::from_model);

    tracing::debug!(
        target: TRACING_TARGET,
        notification_count = response.items.len(),
        "Notifications listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_notifications_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List notifications")
        .description(
            "Returns the authenticated account's notifications, most recent \
             first. Read-only — use POST /notifications/read/ or \
             POST /notifications/{notificationId}/read/ to mark them read.",
        )
        .response::<200, Json<NotificationsPage>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Returns the count of unread notifications for the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_state.account_id)
)]
async fn get_unread_status(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<UnreadStatus>)> {
    tracing::debug!(target: TRACING_TARGET, "Checking unread notifications count");

    let mut conn = pg_client.get_connection().await?;

    let unread_count = conn
        .count_unread_account_notifications(auth_state.account_id)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        unread_count,
        "Unread notifications count retrieved"
    );

    Ok((StatusCode::OK, Json(UnreadStatus { unread_count })))
}

fn get_unread_status_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get unread notifications count")
        .description("Returns the number of unread notifications for the authenticated account.")
        .response::<200, Json<UnreadStatus>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Marks all of the authenticated account's unread notifications as read.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_state.account_id)
)]
async fn mark_all_notifications_read(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<MarkedReadStatus>)> {
    tracing::debug!(target: TRACING_TARGET, "Marking all notifications as read");

    let mut conn = pg_client.get_connection().await?;

    let marked_read = conn
        .mark_all_account_notifications_as_read(auth_state.account_id)
        .await? as i64;

    tracing::debug!(target: TRACING_TARGET, marked_read, "Notifications marked as read");

    Ok((StatusCode::OK, Json(MarkedReadStatus { marked_read })))
}

fn mark_all_notifications_read_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Mark all notifications as read")
        .description(
            "Marks every unread notification for the authenticated account as \
             read and returns how many it marked.",
        )
        .response::<200, Json<MarkedReadStatus>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Marks a single notification as read.
///
/// Scoped to the authenticated account: a notification owned by another account
/// (or a missing id) is reported as not found.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        notification_id = %path_params.notification_id,
    )
)]
async fn mark_notification_read(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<NotificationPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Marking notification as read");

    let mut conn = pg_client.get_connection().await?;

    let marked = conn
        .mark_account_notification_as_read(auth_state.account_id, path_params.notification_id)
        .await?;

    if !marked {
        return Err(Error::not_found("notification"));
    }

    Ok(StatusCode::NO_CONTENT)
}

fn mark_notification_read_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Mark notification as read")
        .description("Marks a single notification of the authenticated account as read.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all notification routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/notifications/",
            get_with(list_notifications, list_notifications_docs),
        )
        .api_route(
            "/notifications/unread/",
            get_with(get_unread_status, get_unread_status_docs),
        )
        .api_route(
            "/notifications/read/",
            post_with(
                mark_all_notifications_read,
                mark_all_notifications_read_docs,
            ),
        )
        .api_route(
            "/notifications/{notificationId}/read/",
            post_with(mark_notification_read, mark_notification_read_docs),
        )
        .with_path_items(|item| item.tag("Notifications"))
}
