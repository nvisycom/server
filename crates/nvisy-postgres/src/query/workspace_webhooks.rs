//! Workspace webhook repository for managing webhook operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceWebhook, UpdateWorkspaceWebhook, WorkspaceWebhook};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, TimeCursor, WebhookEvent, WebhookStatus,
    WithAccountRef, keyset, total,
};
use crate::{PgConnection, Result, schema};

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

    /// Finds a webhook by id within a workspace, with the handle and avatar of
    /// the account that created it, excluding soft-deleted rows.
    fn find_webhook_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        webhook_id: Uuid,
    ) -> impl Future<Output = Result<Option<WithAccountRef<WorkspaceWebhook>>>> + Send;

    /// Lists all webhooks for a workspace with cursor pagination, each paired
    /// with the handle and avatar of the account that created it.
    fn cursor_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
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

    /// Suspends a webhook after repeated delivery failures, but only while it is
    /// still enabled and still has at least `min_failures` consecutive failures.
    ///
    /// The conditions are checked in the same statement, so a status the owner
    /// set, or a failure streak reset by a success, after the failure that
    /// prompted this is never overwritten. Returns the suspended webhook, or
    /// `None` when the conditions no longer hold.
    fn suspend_failing_webhook(
        &mut self,
        webhook_id: Uuid,
        min_failures: i32,
    ) -> impl Future<Output = Result<Option<WorkspaceWebhook>>> + Send;

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

        Ok(diesel::insert_into(workspace_webhooks::table)
            .values(&new_webhook)
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await?)
    }

    async fn find_workspace_webhook_by_id(
        &mut self,
        webhook_id: Uuid,
    ) -> Result<Option<WorkspaceWebhook>> {
        use schema::workspace_webhooks::{self, dsl};

        Ok(workspace_webhooks::table
            .filter(dsl::id.eq(webhook_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceWebhook::as_select())
            .first(self)
            .await
            .optional()?)
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
            .select((WorkspaceWebhook::as_select(), AccountRefRow::as_select()))
            .first::<(WorkspaceWebhook, AccountRefRow)>(self)
            .await
            .optional()?;

        Ok(row.map(WithAccountRef::from))
    }

    async fn cursor_list_workspace_webhooks(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceWebhook>>> {
        use schema::workspace_webhooks::dsl;
        use schema::{accounts, workspace_webhooks};

        let scoped = || {
            workspace_webhooks::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .into_boxed()
        };
        let total = total!(pagination, self, scoped().count());
        let query = scoped().inner_join(accounts::table);

        let after = pagination.after_pair();
        let rows: Vec<(WorkspaceWebhook, AccountRefRow)> =
            keyset!(query, dsl::created_at, dsl::id, pagination.direction, after)
                .select((WorkspaceWebhook::as_select(), AccountRefRow::as_select()))
                .limit(pagination.fetch_limit())
                .load(self)
                .await?;

        let items: Vec<WithAccountRef<WorkspaceWebhook>> =
            rows.into_iter().map(WithAccountRef::from).collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            TimeCursor::new(wc.item.created_at, wc.item.id)
        }))
    }

    async fn update_workspace_webhook(
        &mut self,
        webhook_id: Uuid,
        changes: UpdateWorkspaceWebhook,
    ) -> Result<WorkspaceWebhook> {
        use schema::workspace_webhooks::{self, dsl};

        // Scope to a live row: a concurrently tombstoned webhook yields
        // `NotFound` instead of reviving the dead row.
        Ok(diesel::update(workspace_webhooks::table)
            .filter(dsl::id.eq(webhook_id))
            .filter(dsl::deleted_at.is_null())
            .set(&changes)
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await?)
    }

    async fn delete_workspace_webhook(&mut self, webhook_id: Uuid) -> Result<()> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::{self, dsl};

        // Scope to a live row so re-deleting keeps the original tombstone.
        diesel::update(workspace_webhooks::table)
            .filter(dsl::id.eq(webhook_id))
            .filter(dsl::deleted_at.is_null())
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await?;

        Ok(())
    }

    async fn record_webhook_success(&mut self, webhook_id: Uuid) -> Result<WorkspaceWebhook> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::{self, dsl};

        Ok(diesel::update(workspace_webhooks::table)
            .filter(dsl::id.eq(webhook_id))
            .set((
                dsl::last_success_at.eq(now),
                dsl::consecutive_failures.eq(0),
            ))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await?)
    }

    async fn record_webhook_failure(&mut self, webhook_id: Uuid) -> Result<WorkspaceWebhook> {
        use diesel::dsl::now;
        use schema::workspace_webhooks::{self, dsl};

        Ok(diesel::update(workspace_webhooks::table)
            .filter(dsl::id.eq(webhook_id))
            .set((
                dsl::last_failure_at.eq(now),
                dsl::consecutive_failures.eq(dsl::consecutive_failures + 1),
            ))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await?)
    }

    async fn suspend_failing_webhook(
        &mut self,
        webhook_id: Uuid,
        min_failures: i32,
    ) -> Result<Option<WorkspaceWebhook>> {
        use schema::workspace_webhooks::{self, dsl};

        Ok(diesel::update(workspace_webhooks::table)
            .filter(dsl::id.eq(webhook_id))
            .filter(dsl::status.eq(WebhookStatus::Enabled))
            .filter(dsl::consecutive_failures.ge(min_failures))
            .set(dsl::status.eq(WebhookStatus::Suspended))
            .returning(WorkspaceWebhook::as_returning())
            .get_result(self)
            .await
            .optional()?)
    }

    async fn find_webhooks_for_event(
        &mut self,
        ws_id: Uuid,
        event: WebhookEvent,
    ) -> Result<Vec<WorkspaceWebhook>> {
        use schema::workspace_webhooks::{self, dsl};

        // Query webhooks whose events array contains the target event via the
        // PostgreSQL `@>` operator. The events column is
        // Array<Nullable<WebhookEvent>>, so the bound needle matches that shape.
        let needle: Vec<Option<WebhookEvent>> = vec![Some(event)];

        Ok(workspace_webhooks::table
            .filter(dsl::workspace_id.eq(ws_id))
            .filter(dsl::status.eq(WebhookStatus::Enabled))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::events.contains(needle))
            .select(WorkspaceWebhook::as_select())
            .load(self)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::model::NewWorkspaceWebhook;
    use crate::query::WorkspaceWebhookRepository;
    use crate::test_util::TestDatabase;

    #[tokio::test]
    async fn create_scoped_lookup_and_soft_delete() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let webhook = conn
            .create_workspace_webhook(NewWorkspaceWebhook::test(
                seeded.workspace_id,
                seeded.account_id,
                vec![WebhookEvent::DocumentCreated],
            ))
            .await?;

        assert!(
            conn.find_workspace_webhook_by_id(webhook.id)
                .await?
                .is_some()
        );
        assert!(
            conn.find_webhook_in_workspace_with_creator(seeded.workspace_id, webhook.id)
                .await?
                .is_some()
        );
        // Not found scoped to another workspace.
        assert!(
            conn.find_webhook_in_workspace_with_creator(Uuid::now_v7(), webhook.id)
                .await?
                .is_none()
        );

        // Soft delete hides it from reads.
        conn.delete_workspace_webhook(webhook.id).await?;
        assert!(
            conn.find_workspace_webhook_by_id(webhook.id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn success_resets_failures_and_failure_increments_them() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let webhook = conn
            .create_workspace_webhook(NewWorkspaceWebhook::test(
                seeded.workspace_id,
                seeded.account_id,
                vec![WebhookEvent::DocumentCreated],
            ))
            .await?;

        // Two failures accumulate.
        let _ = conn.record_webhook_failure(webhook.id).await?;
        let after_two = conn.record_webhook_failure(webhook.id).await?;
        assert_eq!(after_two.consecutive_failures, 2);
        assert!(after_two.last_failure_at.is_some());

        // A success resets the counter and records the delivery.
        let after_success = conn.record_webhook_success(webhook.id).await?;
        assert_eq!(after_success.consecutive_failures, 0);
        assert!(after_success.last_success_at.is_some());
        Ok(())
    }

    /// Suspension holds back when the webhook changed after the failure that
    /// prompted it: below the threshold, or no longer enabled.
    #[tokio::test]
    async fn suspension_only_applies_while_still_failing_and_enabled() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let webhook = conn
            .create_workspace_webhook(NewWorkspaceWebhook::test(
                seeded.workspace_id,
                seeded.account_id,
                vec![WebhookEvent::DocumentCreated],
            ))
            .await?;
        conn.record_webhook_failure(webhook.id).await?;
        conn.record_webhook_failure(webhook.id).await?;

        // Below the threshold: left enabled.
        assert!(conn.suspend_failing_webhook(webhook.id, 3).await?.is_none());

        // At the threshold: suspended, once.
        let suspended = conn.suspend_failing_webhook(webhook.id, 2).await?;
        assert_eq!(suspended.map(|w| w.status), Some(WebhookStatus::Suspended));
        assert!(conn.suspend_failing_webhook(webhook.id, 2).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn find_for_event_requires_enabled_subscribed_and_live() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Subscribed and enabled: matches.
        let subscribed = conn
            .create_workspace_webhook(NewWorkspaceWebhook::test(
                seeded.workspace_id,
                seeded.account_id,
                vec![WebhookEvent::DocumentCreated, WebhookEvent::DocumentDeleted],
            ))
            .await?;
        // Subscribed to a different event only: does not match FileCreated.
        let _other_event = conn
            .create_workspace_webhook(NewWorkspaceWebhook::test(
                seeded.workspace_id,
                seeded.account_id,
                vec![WebhookEvent::MemberAdded],
            ))
            .await?;
        // Subscribed but suspended: excluded (not enabled).
        let suspended = conn
            .create_workspace_webhook(NewWorkspaceWebhook::test(
                seeded.workspace_id,
                seeded.account_id,
                vec![WebhookEvent::DocumentCreated],
            ))
            .await?;
        conn.suspend_failing_webhook(suspended.id, 0)
            .await?
            .expect("an enabled webhook is suspended");
        // Subscribed but soft-deleted: excluded.
        let deleted = conn
            .create_workspace_webhook(NewWorkspaceWebhook::test(
                seeded.workspace_id,
                seeded.account_id,
                vec![WebhookEvent::DocumentCreated],
            ))
            .await?;
        conn.delete_workspace_webhook(deleted.id).await?;

        let matched = conn
            .find_webhooks_for_event(seeded.workspace_id, WebhookEvent::DocumentCreated)
            .await?;
        assert_eq!(
            matched.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![subscribed.id]
        );
        Ok(())
    }
}
