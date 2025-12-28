//! Project webhook repository for managing webhook operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectWebhook, ProjectWebhook, UpdateProjectWebhook};
use crate::types::WebhookStatus;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for project webhook database operations.
///
/// Handles webhook management including CRUD operations, status management,
/// and failure tracking.
pub trait ProjectWebhookRepository {
    /// Creates a new project webhook.
    fn create_project_webhook(
        &mut self,
        new_webhook: NewProjectWebhook,
    ) -> impl Future<Output = PgResult<ProjectWebhook>> + Send;

    /// Finds a project webhook by ID.
    fn find_project_webhook_by_id(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectWebhook>>> + Send;

    /// Lists all webhooks for a project.
    fn list_project_webhooks(
        &mut self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectWebhook>>> + Send;

    /// Lists active webhooks for a project.
    fn list_active_project_webhooks(
        &mut self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectWebhook>>> + Send;

    /// Finds webhooks subscribed to a specific event.
    fn find_webhooks_for_event(
        &mut self,
        project_id: Uuid,
        event: &str,
    ) -> impl Future<Output = PgResult<Vec<ProjectWebhook>>> + Send;

    /// Updates a project webhook.
    fn update_project_webhook(
        &mut self,
        webhook_id: Uuid,
        changes: UpdateProjectWebhook,
    ) -> impl Future<Output = PgResult<ProjectWebhook>> + Send;

    /// Soft deletes a project webhook.
    fn delete_project_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Records a successful webhook delivery.
    fn record_webhook_success(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectWebhook>> + Send;

    /// Records a failed webhook delivery.
    fn record_webhook_failure(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectWebhook>> + Send;

    /// Resets the failure count for a webhook.
    fn reset_webhook_failures(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectWebhook>> + Send;

    /// Pauses a webhook.
    fn pause_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectWebhook>> + Send;

    /// Resumes a paused webhook.
    fn resume_webhook(
        &mut self,
        webhook_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectWebhook>> + Send;
}

impl ProjectWebhookRepository for PgConnection {
    async fn create_project_webhook(
        &mut self,
        new_webhook: NewProjectWebhook,
    ) -> PgResult<ProjectWebhook> {
        use schema::project_webhooks;

        let webhook = diesel::insert_into(project_webhooks::table)
            .values(&new_webhook)
            .returning(ProjectWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn find_project_webhook_by_id(
        &mut self,
        webhook_id: Uuid,
    ) -> PgResult<Option<ProjectWebhook>> {
        use schema::project_webhooks::dsl::*;

        let webhook = project_webhooks
            .filter(id.eq(webhook_id))
            .filter(deleted_at.is_null())
            .select(ProjectWebhook::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn list_project_webhooks(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectWebhook>> {
        use schema::project_webhooks::dsl::*;

        let webhooks = project_webhooks
            .filter(project_id.eq(proj_id))
            .filter(deleted_at.is_null())
            .select(ProjectWebhook::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhooks)
    }

    async fn list_active_project_webhooks(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectWebhook>> {
        use schema::project_webhooks::dsl::*;

        let webhooks = project_webhooks
            .filter(project_id.eq(proj_id))
            .filter(status.eq(WebhookStatus::Active))
            .filter(deleted_at.is_null())
            .select(ProjectWebhook::as_select())
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
        event: &str,
    ) -> PgResult<Vec<ProjectWebhook>> {
        use schema::project_webhooks::dsl::*;

        let webhooks = project_webhooks
            .filter(project_id.eq(proj_id))
            .filter(status.eq(WebhookStatus::Active))
            .filter(events.contains(vec![Some(event.to_string())]))
            .filter(deleted_at.is_null())
            .select(ProjectWebhook::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhooks)
    }

    async fn update_project_webhook(
        &mut self,
        webhook_id: Uuid,
        changes: UpdateProjectWebhook,
    ) -> PgResult<ProjectWebhook> {
        use schema::project_webhooks::dsl::*;

        let webhook = diesel::update(project_webhooks)
            .filter(id.eq(webhook_id))
            .set(&changes)
            .returning(ProjectWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn delete_project_webhook(&mut self, webhook_id: Uuid) -> PgResult<()> {
        use schema::project_webhooks::dsl::*;

        diesel::update(project_webhooks)
            .filter(id.eq(webhook_id))
            .set(deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn record_webhook_success(&mut self, webhook_id: Uuid) -> PgResult<ProjectWebhook> {
        use schema::project_webhooks::dsl::*;

        let now = jiff_diesel::Timestamp::from(Timestamp::now());
        let webhook = diesel::update(project_webhooks)
            .filter(id.eq(webhook_id))
            .set((
                failure_count.eq(0),
                last_triggered_at.eq(Some(now)),
                last_success_at.eq(Some(now)),
                status.eq(WebhookStatus::Active),
            ))
            .returning(ProjectWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn record_webhook_failure(&mut self, webhook_id: Uuid) -> PgResult<ProjectWebhook> {
        use schema::project_webhooks::dsl::*;

        let now = jiff_diesel::Timestamp::from(Timestamp::now());

        // First get the current webhook to check failure count
        let current = project_webhooks
            .filter(id.eq(webhook_id))
            .select(ProjectWebhook::as_select())
            .first(self)
            .await
            .map_err(PgError::from)?;

        let new_failure_count = current.failure_count + 1;
        let new_status = if new_failure_count >= current.max_failures {
            WebhookStatus::Disabled
        } else {
            current.status
        };

        let webhook = diesel::update(project_webhooks)
            .filter(id.eq(webhook_id))
            .set((
                failure_count.eq(new_failure_count),
                last_triggered_at.eq(Some(now)),
                last_failure_at.eq(Some(now)),
                status.eq(new_status),
            ))
            .returning(ProjectWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn reset_webhook_failures(&mut self, webhook_id: Uuid) -> PgResult<ProjectWebhook> {
        use schema::project_webhooks::dsl::*;

        let webhook = diesel::update(project_webhooks)
            .filter(id.eq(webhook_id))
            .set((failure_count.eq(0), status.eq(WebhookStatus::Active)))
            .returning(ProjectWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn pause_webhook(&mut self, webhook_id: Uuid) -> PgResult<ProjectWebhook> {
        use schema::project_webhooks::dsl::*;

        let webhook = diesel::update(project_webhooks)
            .filter(id.eq(webhook_id))
            .set(status.eq(WebhookStatus::Paused))
            .returning(ProjectWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }

    async fn resume_webhook(&mut self, webhook_id: Uuid) -> PgResult<ProjectWebhook> {
        use schema::project_webhooks::dsl::*;

        let webhook = diesel::update(project_webhooks)
            .filter(id.eq(webhook_id))
            .set(status.eq(WebhookStatus::Active))
            .returning(ProjectWebhook::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(webhook)
    }
}
