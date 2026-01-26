//! Workspace webhook repository for managing webhook operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceWebhook, UpdateWorkspaceWebhook, WorkspaceWebhook};
use crate::types::{
    Cursor, CursorPage, CursorPagination, OffsetPagination, WebhookEvent, WebhookStatus,
};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace webhook database operations.
///
/// Handles webhook management including CRUD operations and status management.
pub trait WorkspaceWebhookRepository {
    /// Creates a new workspace webhook.
    fn create_workspace_webhook(
        &mut self,
        new_webhook: NewWorkspaceWebhook,
    ) -> impl Future<Output = PgResult<WorkspaceWebhook>> + Send;

    /// Finds a workspace webhook by ID.
    fn find_workspace_webhook_by_id(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceWebhook>>> + Send;

    /// Lists all webhooks for a workspace with offset pagination.
    fn offset_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceWebhook>>> + Send;

    /// Lists all webhooks for a workspace with cursor pagination.
    fn cursor_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceWebhook>>> + Send;

    /// Updates a workspace webhook.
    fn update_workspace_webhook(
        &mut self,
        webhook_id: Uuid,
        changes: UpdateWorkspaceWebhook,
    ) -> impl Future<Output = PgResult<WorkspaceWebhook>> + Send;

    /// Soft deletes a workspace webhook.
    fn delete_workspace_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Records a successful webhook delivery.
    fn record_webhook_success(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceWebhook>> + Send;

    /// Records a failed webhook delivery.
    fn record_webhook_failure(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceWebhook>> + Send;

    /// Pauses a webhook.
    fn pause_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceWebhook>> + Send;

    /// Resumes a paused webhook.
    fn resume_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceWebhook>> + Send;

    /// Disables a webhook.
    fn disable_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceWebhook>> + Send;

    /// Finds all active webhooks for a workspace that are subscribed to a specific event.
    ///
    /// Returns webhooks where:
    /// - The webhook belongs to the specified workspace
    /// - The webhook status is Active
    /// - The webhook's events array contains the specified event
    /// - The webhook is not deleted
    fn find_webhooks_for_event(
        &mut self,
        workspace_id: Uuid,
        event: WebhookEvent,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceWebhook>>> + Send;
}

impl WorkspaceWebhookRepository for PgConnection {
    async fn create_workspace_webhook(
        &mut self,
        new_webhook: NewWorkspaceWebhook,
    ) -> PgResult<WorkspaceWebhook> {
        use schema::workspace_webhooks;

        let webhook = diesel::insert_into(workspace_webhooks::table)
            .values(&new_webhook)
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn find_workspace_webhook_by_id(
        &mut self,
        webhook_id: Uuid,
    ) -> PgResult<Option<WorkspaceWebhook>> {
        use schema::workspace_webhooks::dsl::*;

        let webhook = workspace_webhooks
            .filter(id.eq(webhook_id))
            .filter(deleted_at.is_null())
            .select(WorkspaceWebhook::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn offset_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceWebhook>> {
        use schema::workspace_webhooks::{self, dsl};

        let webhooks = workspace_webhooks::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceWebhook::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhooks)
    }

    async fn cursor_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WorkspaceWebhook>> {
        use schema::workspace_webhooks::{self, dsl};

        // Get total count only if requested
        let total = if pagination.include_count {
            Some(
                workspace_webhooks::table
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .filter(dsl::deleted_at.is_null())
                    .count()
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Build query with cursor
        let mut query = workspace_webhooks::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            query = query.filter(
                dsl::created_at
                    .lt(cursor_ts)
                    .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
            );
        }

        let fetch_limit = pagination.fetch_limit();
        let mut items: Vec<WorkspaceWebhook> = query
            .select(WorkspaceWebhook::as_select())
            .order((dsl::created_at.desc(), dsl::id.desc()))
            .limit(fetch_limit)
            .load(self)
            .await
            .map_err(PgError::from)?;

        let has_more = items.len() as i64 > pagination.limit;
        if has_more {
            items.pop();
        }

        let next_cursor = if has_more {
            items.last().map(|w| {
                Cursor {
                    timestamp: w.created_at.into(),
                    id: w.id,
                }
                .encode()
            })
        } else {
            None
        };

        Ok(CursorPage {
            items,
            total,
            next_cursor,
        })
    }

    async fn update_workspace_webhook(
        &mut self,
        webhook_id: Uuid,
        changes: UpdateWorkspaceWebhook,
    ) -> PgResult<WorkspaceWebhook> {
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(&changes)
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn delete_workspace_webhook(&mut self, webhook_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::dsl::*;

        diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn record_webhook_success(&mut self, webhook_id: Uuid) -> PgResult<WorkspaceWebhook> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(last_triggered_at.eq(now))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn record_webhook_failure(&mut self, webhook_id: Uuid) -> PgResult<WorkspaceWebhook> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(last_triggered_at.eq(now))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn pause_webhook(&mut self, webhook_id: Uuid) -> PgResult<WorkspaceWebhook> {
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(status.eq(WebhookStatus::Paused))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn resume_webhook(&mut self, webhook_id: Uuid) -> PgResult<WorkspaceWebhook> {
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(status.eq(WebhookStatus::Active))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn disable_webhook(&mut self, webhook_id: Uuid) -> PgResult<WorkspaceWebhook> {
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(status.eq(WebhookStatus::Disabled))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn find_webhooks_for_event(
        &mut self,
        ws_id: Uuid,
        event: WebhookEvent,
    ) -> PgResult<Vec<WorkspaceWebhook>> {
        use diesel::dsl::sql;
        use diesel::sql_types::Bool;
        use schema::workspace_webhooks::dsl::*;

        // Query webhooks where the events array contains the target event.
        // Uses PostgreSQL's `@>` (array contains) operator via raw SQL.
        // The events column is Array<Nullable<WebhookEvent>>, so we check if
        // the array contains the event value.
        let event_str = format!("'{}'", event.to_string().replace('\'', "''"));
        let contains_event =
            sql::<Bool>(&format!("events @> ARRAY[{}]::WEBHOOK_EVENT[]", event_str));

        let webhooks = workspace_webhooks
            .filter(workspace_id.eq(ws_id))
            .filter(status.eq(WebhookStatus::Active))
            .filter(deleted_at.is_null())
            .filter(contains_event)
            .select(WorkspaceWebhook::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhooks)
    }
}
