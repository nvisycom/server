//! Account notification handlers.
//!
//! This module provides handlers for viewing account notifications.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::http::StatusCode;
use nvisy_postgres::query::AccountNotificationRepository;

use crate::extract::{AuthState, Json, PgPool, Query};
use crate::handler::Result;
use crate::handler::request::Pagination;
use crate::handler::response::{ErrorResponse, Notification, Notifications, UnreadStatus};
use crate::service::ServiceState;

/// Tracing target for notification operations.
const TRACING_TARGET: &str = "nvisy_server::handler::notifications";

/// Lists notifications for the authenticated account and marks them as read.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_state.account_id)
)]
async fn list_notifications(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Query(pagination): Query<Pagination>,
) -> Result<(StatusCode, Json<Notifications>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing notifications");

    let notifications = conn
        .find_notifications_by_account(auth_state.account_id, pagination.into())
        .await?;

    // Mark all unread notifications as read
    let unread_count = conn.mark_all_as_read(auth_state.account_id).await?;

    if unread_count > 0 {
        tracing::debug!(
            target: TRACING_TARGET,
            unread_count,
            "Marked notifications as read"
        );
    }

    let notifications: Notifications = Notification::from_models(notifications);

    tracing::debug!(
        target: TRACING_TARGET,
        notification_count = notifications.len(),
        "Notifications listed"
    );

    Ok((StatusCode::OK, Json(notifications)))
}

fn list_notifications_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List notifications")
        .description(
            "Returns all notifications for the authenticated account and marks them as read.",
        )
        .response::<200, Json<Notifications>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Returns the count of unread notifications for the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_state.account_id)
)]
async fn get_unread_status(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<UnreadStatus>)> {
    tracing::debug!(target: TRACING_TARGET, "Checking unread notifications count");

    let unread_count = conn
        .count_unread_notifications(auth_state.account_id)
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

/// Returns routes for notification management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/notifications/",
            get_with(list_notifications, list_notifications_docs),
        )
        .api_route(
            "/notifications/unread",
            get_with(get_unread_status, get_unread_status_docs),
        )
        .with_path_items(|item| item.tag("Notifications"))
}
