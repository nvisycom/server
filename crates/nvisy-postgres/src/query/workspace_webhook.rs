//! Workspace webhook repository for managing webhook operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceWebhook, UpdateWorkspaceWebhook, WorkspaceWebhook};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, OffsetPagination, WebhookEvent, WebhookStatus,
    WithAccountRef,
};
use crate::{Error, PgConnection, Result, schema};

/// Repository for workspace webhook database operations.
///
/// Handles webhook management including CRUD operations and status management.
pub trait WorkspaceWebhookRepository {
    /// Creates a new workspace webhook.
    fn create_workspace_webhook(
        &mut self,
        new_webhook: NewWorkspaceWebhook,
    ) -> impl Future<Output = Result<WorkspaceWebhook>> + Send;

    /// Finds a workspace webhook by ID.
    fn find_workspace_webhook_by_id(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceWebhook>>> + Send;

    /// Finds a webhook by ID, scoped to its workspace.
    fn find_webhook_in_workspace(
        &mut self,
        workspace_id: Uuid,
        webhook_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceWebhook>>> + Send;

    /// Finds a webhook by id within a workspace, with the handle and avatar of
    /// the account that created it, excluding soft-deleted rows.
    fn find_webhook_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        webhook_id: Uuid,
    ) -> impl Future<Output = Result<Option<WithAccountRef<WorkspaceWebhook>>>> + Send;

    /// Lists all webhooks for a workspace with offset pagination.
    fn offset_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<WorkspaceWebhook>>> + Send;

    /// Lists all webhooks for a workspace with cursor pagination, each paired
    /// with the handle and avatar of the account that created it.
    fn cursor_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceWebhook>>>> + Send;

    /// Updates a workspace webhook.
    fn update_workspace_webhook(
        &mut self,
        webhook_id: Uuid,
        changes: UpdateWorkspaceWebhook,
    ) -> impl Future<Output = Result<WorkspaceWebhook>> + Send;

    /// Soft deletes a workspace webhook.
    fn delete_workspace_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Records a successful webhook delivery.
    fn record_webhook_success(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = Result<WorkspaceWebhook>> + Send;

    /// Records a failed webhook delivery.
    fn record_webhook_failure(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = Result<WorkspaceWebhook>> + Send;

    /// Suspends a webhook (system-set, e.g. after repeated failures).
    fn suspend_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = Result<WorkspaceWebhook>> + Send;

    /// Finds all enabled webhooks for a workspace that are subscribed to a specific event.
    ///
    /// Returns webhooks where:
    /// - The webhook belongs to the specified workspace
    /// - The webhook status is Enabled
    /// - The webhook's events array contains the specified event
    /// - The webhook is not deleted
    fn find_webhooks_for_event(
        &mut self,
        workspace_id: Uuid,
        event: WebhookEvent,
    ) -> impl Future<Output = Result<Vec<WorkspaceWebhook>>> + Send;
}

impl WorkspaceWebhookRepository for PgConnection {
    async fn create_workspace_webhook(
        &mut self,
        new_webhook: NewWorkspaceWebhook,
    ) -> Result<WorkspaceWebhook> {
        use schema::workspace_webhooks;

        let webhook = diesel::insert_into(workspace_webhooks::table)
            .values(&new_webhook)
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(webhook)
    }

    async fn find_workspace_webhook_by_id(
        &mut self,
        webhook_id: Uuid,
    ) -> Result<Option<WorkspaceWebhook>> {
        use schema::workspace_webhooks::dsl::*;

        let webhook = workspace_webhooks
            .filter(id.eq(webhook_id))
            .filter(deleted_at.is_null())
            .select(WorkspaceWebhook::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(webhook)
    }

    async fn find_webhook_in_workspace(
        &mut self,
        workspace_id: Uuid,
        webhook_id: Uuid,
    ) -> Result<Option<WorkspaceWebhook>> {
        use schema::workspace_webhooks::{self, dsl};

        let webhook = workspace_webhooks::table
            .filter(dsl::id.eq(webhook_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceWebhook::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(webhook)
    }

    async fn find_webhook_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        webhook_id: Uuid,
    ) -> Result<Option<WithAccountRef<WorkspaceWebhook>>> {
        use schema::workspace_webhooks::dsl;
        use schema::{accounts, workspace_webhooks};

        let row = workspace_webhooks::table
            .inner_join(accounts::table)
            .filter(dsl::id.eq(webhook_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspaceWebhook::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .first::<(WorkspaceWebhook, AccountRefRow)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|(item, account)| WithAccountRef { item, account }))
    }

    async fn offset_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> Result<Vec<WorkspaceWebhook>> {
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
            .map_err(Error::from)?;

        Ok(webhooks)
    }

    async fn cursor_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceWebhook>>> {
        use schema::workspace_webhooks::dsl;
        use schema::{accounts, workspace_webhooks};

        // Get total count only if requested
        let total = if pagination.include_count {
            Some(
                workspace_webhooks::table
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .filter(dsl::deleted_at.is_null())
                    .count()
                    .get_result(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        // Build query with cursor
        let mut query = workspace_webhooks::table
            .inner_join(accounts::table)
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

        let rows: Vec<(WorkspaceWebhook, AccountRefRow)> = query
            .select((
                WorkspaceWebhook::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .order((dsl::created_at.desc(), dsl::id.desc()))
            .limit(pagination.fetch_limit())
            .load(self)
            .await
            .map_err(Error::from)?;

        let items: Vec<WithAccountRef<WorkspaceWebhook>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            (wc.item.created_at.into(), wc.item.id)
        }))
    }

    async fn update_workspace_webhook(
        &mut self,
        webhook_id: Uuid,
        changes: UpdateWorkspaceWebhook,
    ) -> Result<WorkspaceWebhook> {
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(&changes)
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(webhook)
    }

    async fn delete_workspace_webhook(&mut self, webhook_id: Uuid) -> Result<()> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::dsl::*;

        diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn record_webhook_success(&mut self, webhook_id: Uuid) -> Result<WorkspaceWebhook> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set((last_success_at.eq(now), consecutive_failures.eq(0)))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(webhook)
    }

    async fn record_webhook_failure(&mut self, webhook_id: Uuid) -> Result<WorkspaceWebhook> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set((
                last_failure_at.eq(now),
                consecutive_failures.eq(consecutive_failures + 1),
            ))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(webhook)
    }

    async fn suspend_webhook(&mut self, webhook_id: Uuid) -> Result<WorkspaceWebhook> {
        use schema::workspace_webhooks::dsl::*;

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(status.eq(WebhookStatus::Suspended))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(webhook)
    }

    async fn find_webhooks_for_event(
        &mut self,
        ws_id: Uuid,
        event: WebhookEvent,
    ) -> Result<Vec<WorkspaceWebhook>> {
        use schema::workspace_webhooks::dsl::*;

        // Query webhooks whose events array contains the target event via the
        // PostgreSQL `@>` operator. The events column is
        // Array<Nullable<WebhookEvent>>, so the bound needle matches that shape.
        let needle: Vec<Option<WebhookEvent>> = vec![Some(event)];

        let webhooks = workspace_webhooks
            .filter(workspace_id.eq(ws_id))
            .filter(status.eq(WebhookStatus::Enabled))
            .filter(deleted_at.is_null())
            .filter(events.contains(needle))
            .select(WorkspaceWebhook::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(webhooks)
    }
}
