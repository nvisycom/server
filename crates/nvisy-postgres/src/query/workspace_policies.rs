//! Workspace policies repository for managing redaction policy config.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{NewWorkspacePolicy, UpdateWorkspacePolicy, WorkspacePolicy};
use crate::types::{AccountRefRow, CursorPage, CursorPagination, WithAccountRef, keyset};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating a workspace's policies: newest first by `created_at`,
/// `id` as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCursor {
    /// When the policy was created.
    pub created_at: Timestamp,
    /// Policy id (tiebreaker).
    pub id: uuid::Uuid,
}

/// Repository for workspace policy database operations.
pub trait WorkspacePolicyRepository {
    /// Creates a new workspace policy record.
    fn create_workspace_policy(
        &mut self,
        new_policy: NewWorkspacePolicy,
    ) -> impl Future<Output = Result<WorkspacePolicy>> + Send;

    /// Finds a policy by ID within a specific workspace.
    fn find_policy_in_workspace(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspacePolicy>>> + Send;

    /// Finds a policy by slug within a specific workspace, with the handle and
    /// avatar of the account that created it.
    fn find_policy_in_workspace_by_slug(
        &mut self,
        workspace_id: Uuid,
        slug: &str,
    ) -> impl Future<Output = Result<Option<WithAccountRef<WorkspacePolicy>>>> + Send;

    /// Lists all policies in a workspace with cursor pagination, each paired
    /// with the handle and avatar of the account that created it.
    fn cursor_list_workspace_policies(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<PolicyCursor>,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspacePolicy>>>> + Send;

    /// Updates a policy with new data.
    fn update_workspace_policy(
        &mut self,
        policy_id: Uuid,
        updates: UpdateWorkspacePolicy,
    ) -> impl Future<Output = Result<WorkspacePolicy>> + Send;

    /// Soft deletes a policy by setting the deletion timestamp.
    fn delete_workspace_policy(
        &mut self,
        policy_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspacePolicyRepository for PgConnection {
    async fn create_workspace_policy(
        &mut self,
        new_policy: NewWorkspacePolicy,
    ) -> Result<WorkspacePolicy> {
        use schema::workspace_policies;

        let policy = diesel::insert_into(workspace_policies::table)
            .values(&new_policy)
            .returning(WorkspacePolicy::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(policy)
    }

    async fn find_policy_in_workspace(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
    ) -> Result<Option<WorkspacePolicy>> {
        use schema::workspace_policies::{self, dsl};

        let policy = workspace_policies::table
            .filter(dsl::id.eq(policy_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspacePolicy::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(policy)
    }

    async fn find_policy_in_workspace_by_slug(
        &mut self,
        workspace_id: Uuid,
        slug: &str,
    ) -> Result<Option<WithAccountRef<WorkspacePolicy>>> {
        use schema::workspace_policies::dsl;
        use schema::{accounts, workspace_policies};

        let row = workspace_policies::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::slug.eq(slug))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspacePolicy::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .first::<(WorkspacePolicy, AccountRefRow)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|(item, account)| WithAccountRef { item, account }))
    }

    async fn cursor_list_workspace_policies(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<PolicyCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspacePolicy>>> {
        use schema::workspace_policies::dsl;
        use schema::{accounts, workspace_policies};

        let total = if pagination.include_count {
            Some(
                workspace_policies::table
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .filter(dsl::deleted_at.is_null())
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let query = workspace_policies::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let rows: Vec<(WorkspacePolicy, AccountRefRow)> =
            keyset!(query, dsl::created_at, dsl::id, pagination.direction, after)
                .select((
                    WorkspacePolicy::as_select(),
                    (
                        accounts::username,
                        accounts::display_name,
                        accounts::avatar_url,
                    ),
                ))
                .limit(pagination.fetch_limit())
                .load(self)
                .await
                .map_err(Error::from)?;

        let items: Vec<WithAccountRef<WorkspacePolicy>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            PolicyCursor {
                created_at: wc.item.created_at.into(),
                id: wc.item.id,
            }
        }))
    }

    async fn update_workspace_policy(
        &mut self,
        policy_id: Uuid,
        updates: UpdateWorkspacePolicy,
    ) -> Result<WorkspacePolicy> {
        use schema::workspace_policies::{self, dsl};

        let policy = diesel::update(workspace_policies::table.filter(dsl::id.eq(policy_id)))
            .set(&updates)
            .returning(WorkspacePolicy::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(policy)
    }

    async fn delete_workspace_policy(&mut self, policy_id: Uuid) -> Result<()> {
        use diesel::dsl::now;
        use schema::workspace_policies::{self, dsl};

        diesel::update(workspace_policies::table.filter(dsl::id.eq(policy_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};
    use uuid::Uuid;

    use super::*;
    use crate::model::{NewWorkspacePolicy, UpdateWorkspacePolicy};
    use crate::query::WorkspacePolicyRepository;
    use crate::test_util::{TestDatabase, backdate};

    #[tokio::test]
    async fn create_find_update_and_soft_delete() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let policy = conn
            .create_workspace_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let slug = policy.slug.as_str().to_owned();

        // Found by id and by slug within the workspace.
        assert!(
            conn.find_policy_in_workspace(seeded.workspace_id, policy.id)
                .await?
                .is_some()
        );
        let by_slug = conn
            .find_policy_in_workspace_by_slug(seeded.workspace_id, &slug)
            .await?;
        assert_eq!(by_slug.map(|p| p.item.id), Some(policy.id));

        // Not found in another workspace.
        assert!(
            conn.find_policy_in_workspace(Uuid::now_v7(), policy.id)
                .await?
                .is_none()
        );

        // Update the display name.
        let updated = conn
            .update_workspace_policy(
                policy.id,
                UpdateWorkspacePolicy {
                    display_name: Some("Renamed Policy".to_owned()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(updated.display_name, "Renamed Policy");

        // Soft delete hides it from both lookups.
        conn.delete_workspace_policy(policy.id).await?;
        assert!(
            conn.find_policy_in_workspace(seeded.workspace_id, policy.id)
                .await?
                .is_none()
        );
        assert!(
            conn.find_policy_in_workspace_by_slug(seeded.workspace_id, &slug)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_returns_live_policies_newest_first() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Backdate `first` an hour so it is unambiguously older than `second`;
        // without a distinct `created_at` the two could tie and the newest-first
        // order would not be well-defined.
        let first = conn
            .create_workspace_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        backdate::policy_created_at(&mut conn, first.id, Timestamp::now() - Span::new().hours(1))
            .await?;
        let second = conn
            .create_workspace_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let deleted = conn
            .create_workspace_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        conn.delete_workspace_policy(deleted.id).await?;

        let page = conn
            .cursor_list_workspace_policies(seeded.workspace_id, CursorPagination::new(50))
            .await?;
        assert_eq!(
            page.items.iter().map(|p| p.item.id).collect::<Vec<_>>(),
            vec![second.id, first.id]
        );
        Ok(())
    }
}
