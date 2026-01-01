//! Workspace webhook repository for managing webhook operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewWorkspaceWebhook, UpdateWorkspaceWebhook, WorkspaceWebhook};
use crate::types::{WebhookEvent, WebhookStatus};
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

    /// Lists all webhooks for a workspace.
    fn list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceWebhook>>> + Send;

    /// Lists active webhooks for a workspace.
    fn list_active_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceWebhook>>> + Send;

    /// Finds webhooks subscribed to a specific event.
    fn find_webhooks_for_event(
        &mut self,
        workspace_id: Uuid,
        event: WebhookEvent,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceWebhook>>> + Send;

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

    async fn list_workspace_webhooks(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceWebhook>> {
        use schema::workspace_webhooks::dsl::*;

        let webhooks = workspace_webhooks
            .filter(workspace_id.eq(proj_id))
            .filter(deleted_at.is_null())
            .select(WorkspaceWebhook::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhooks)
    }

    async fn list_active_workspace_webhooks(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceWebhook>> {
        use schema::workspace_webhooks::dsl::*;

        let webhooks = workspace_webhooks
            .filter(workspace_id.eq(proj_id))
            .filter(status.eq(WebhookStatus::Active))
            .filter(deleted_at.is_null())
            .select(WorkspaceWebhook::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhooks)
    }

    async fn find_webhooks_for_event(
        &mut self,
        proj_id: Uuid,
        event: WebhookEvent,
    ) -> PgResult<Vec<WorkspaceWebhook>> {
        use schema::workspace_webhooks::dsl::*;

        let webhooks = workspace_webhooks
            .filter(workspace_id.eq(proj_id))
            .filter(status.eq(WebhookStatus::Active))
            .filter(events.contains(vec![Some(event)]))
            .filter(deleted_at.is_null())
            .select(WorkspaceWebhook::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhooks)
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
        use schema::workspace_webhooks::dsl::*;

        diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set(deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn record_webhook_success(&mut self, webhook_id: Uuid) -> PgResult<WorkspaceWebhook> {
        use schema::workspace_webhooks::dsl::*;

        let now = jiff_diesel::Timestamp::from(Timestamp::now());
        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set((
                last_triggered_at.eq(Some(now)),
                last_success_at.eq(Some(now)),
            ))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn record_webhook_failure(&mut self, webhook_id: Uuid) -> PgResult<WorkspaceWebhook> {
        use schema::workspace_webhooks::dsl::*;

        let now = jiff_diesel::Timestamp::from(Timestamp::now());

        let webhook = diesel::update(workspace_webhooks)
            .filter(id.eq(webhook_id))
            .set((
                last_triggered_at.eq(Some(now)),
                last_failure_at.eq(Some(now)),
            ))
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
}
