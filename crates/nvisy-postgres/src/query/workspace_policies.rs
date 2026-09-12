//! Workspace policies repository for managing redaction policy config.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    NewWorkspacePolicy, NewWorkspacePolicyVersion, UpdateWorkspacePolicy, WorkspacePolicy,
    WorkspacePolicyVersion,
};
use crate::types::{AccountRefRow, CursorPage, CursorPagination, WithAccountRef, keyset};
use crate::{Error, PgConnection, Result, schema};

/// A logical policy paired with its current version's definition.
#[derive(Debug, Clone)]
pub struct PolicyWithVersion {
    /// The logical policy.
    pub policy: WorkspacePolicy,
    /// The live version whose definition the engine consumes.
    pub version: WorkspacePolicyVersion,
}

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
    /// Creates a logical policy and its first version (definition + versioned
    /// metadata), pointing the policy at that version — all in one transaction.
    fn create_workspace_policy(
        &mut self,
        new_policy: NewWorkspacePolicy,
        definition: Vec<u8>,
        version_metadata: Option<serde_json::Value>,
    ) -> impl Future<Output = Result<PolicyWithVersion>> + Send;

    /// Creates a logical policy with a placeholder definition version, returning
    /// just the logical policy — a convenience for tests that do not exercise the
    /// definition.
    #[cfg(any(feature = "test_util", test))]
    fn create_test_policy(
        &mut self,
        new_policy: NewWorkspacePolicy,
    ) -> impl Future<Output = Result<WorkspacePolicy>> + Send
    where
        Self: Sized + Send,
    {
        async move {
            let created = self
                .create_workspace_policy(new_policy, vec![1, 2, 3], None)
                .await?;
            Ok(created.policy)
        }
    }

    /// Records a new version of a policy's definition and repoints the policy at
    /// it, in one transaction. The new version's number is one past the policy's
    /// current version. Editing a definition mints a version rather than mutating
    /// the frozen one a detection may have pinned.
    fn create_policy_version(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
        account_id: Uuid,
        definition: Vec<u8>,
        version_metadata: Option<serde_json::Value>,
    ) -> impl Future<Output = Result<WorkspacePolicyVersion>> + Send;

    /// Finds a live policy and its current version within a workspace.
    fn find_policy_with_version(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
    ) -> impl Future<Output = Result<Option<PolicyWithVersion>>> + Send;

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
        definition: Vec<u8>,
        version_metadata: Option<serde_json::Value>,
    ) -> Result<PolicyWithVersion> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_policies, workspace_policy_versions};

        self.transaction(async |conn| {
            let policy = diesel::insert_into(workspace_policies::table)
                .values(&new_policy)
                .returning(WorkspacePolicy::as_returning())
                .get_result::<WorkspacePolicy>(conn)
                .await
                .map_err(Error::from)?;

            let version = diesel::insert_into(workspace_policy_versions::table)
                .values(NewWorkspacePolicyVersion {
                    policy_id: policy.id,
                    workspace_id: policy.workspace_id,
                    account_id: policy.account_id,
                    version_number: 1,
                    definition,
                    metadata: version_metadata,
                })
                .returning(WorkspacePolicyVersion::as_returning())
                .get_result::<WorkspacePolicyVersion>(conn)
                .await
                .map_err(Error::from)?;

            let policy = diesel::update(
                workspace_policies::table.filter(workspace_policies::id.eq(policy.id)),
            )
            .set(workspace_policies::current_version_id.eq(version.id))
            .returning(WorkspacePolicy::as_returning())
            .get_result::<WorkspacePolicy>(conn)
            .await
            .map_err(Error::from)?;

            Ok(PolicyWithVersion { policy, version })
        })
        .await
    }

    async fn create_policy_version(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
        account_id: Uuid,
        definition: Vec<u8>,
        version_metadata: Option<serde_json::Value>,
    ) -> Result<WorkspacePolicyVersion> {
        use diesel::dsl::max;
        use diesel_async::AsyncConnection;
        use schema::{workspace_policies, workspace_policy_versions};

        self.transaction(async |conn| {
            // Lock the parent policy for the transaction so concurrent definition
            // edits serialize: each computes the next version_number against a stable
            // max and cannot collide on the (policy_id, version_number) unique index.
            workspace_policies::table
                .filter(workspace_policies::id.eq(policy_id))
                .filter(workspace_policies::deleted_at.is_null())
                .select(workspace_policies::id)
                .for_update()
                .first::<Uuid>(conn)
                .await
                .map_err(Error::from)?;

            let next_number = workspace_policy_versions::table
                .filter(workspace_policy_versions::policy_id.eq(policy_id))
                .select(max(workspace_policy_versions::version_number))
                .first::<Option<i32>>(conn)
                .await
                .map_err(Error::from)?
                .unwrap_or(0)
                + 1;

            let version = diesel::insert_into(workspace_policy_versions::table)
                .values(NewWorkspacePolicyVersion {
                    policy_id,
                    workspace_id,
                    account_id,
                    version_number: next_number,
                    definition,
                    metadata: version_metadata,
                })
                .returning(WorkspacePolicyVersion::as_returning())
                .get_result::<WorkspacePolicyVersion>(conn)
                .await
                .map_err(Error::from)?;

            diesel::update(
                workspace_policies::table
                    .filter(workspace_policies::id.eq(policy_id))
                    .filter(workspace_policies::deleted_at.is_null()),
            )
            .set(workspace_policies::current_version_id.eq(version.id))
            .execute(conn)
            .await
            .map_err(Error::from)?;

            Ok(version)
        })
        .await
    }

    async fn find_policy_with_version(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
    ) -> Result<Option<PolicyWithVersion>> {
        use schema::{workspace_policies, workspace_policy_versions};

        let row = workspace_policies::table
            .inner_join(workspace_policy_versions::table.on(
                workspace_policies::current_version_id.eq(workspace_policy_versions::id.nullable()),
            ))
            .filter(workspace_policies::id.eq(policy_id))
            .filter(workspace_policies::workspace_id.eq(workspace_id))
            .filter(workspace_policies::deleted_at.is_null())
            .select((
                WorkspacePolicy::as_select(),
                WorkspacePolicyVersion::as_select(),
            ))
            .first::<(WorkspacePolicy, WorkspacePolicyVersion)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|(policy, version)| PolicyWithVersion { policy, version }))
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

        // Scope to a live row so a concurrently soft-deleted policy is not
        // revived by the update.
        let policy = diesel::update(
            workspace_policies::table
                .filter(dsl::id.eq(policy_id))
                .filter(dsl::deleted_at.is_null()),
        )
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
    use crate::query::{WorkspacePolicyRepository, WorkspacePolicyVersionRepository};
    use crate::test_util::{TestDatabase, backdate};

    #[tokio::test]
    async fn create_find_update_and_soft_delete() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let policy = conn
            .create_test_policy(NewWorkspacePolicy::test(
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
            .create_test_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        backdate::policy_created_at(&mut conn, first.id, Timestamp::now() - Span::new().hours(1))
            .await?;
        let second = conn
            .create_test_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let deleted = conn
            .create_test_policy(NewWorkspacePolicy::test(
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

    #[tokio::test]
    async fn create_makes_version_one_and_points_current() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let created = conn
            .create_workspace_policy(
                NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id),
                vec![1, 2, 3],
                None,
            )
            .await?;
        assert_eq!(created.version.version_number, 1);
        assert_eq!(created.policy.current_version_id, Some(created.version.id));
        assert_eq!(created.version.definition, vec![1, 2, 3]);

        let found = conn
            .find_policy_with_version(seeded.workspace_id, created.policy.id)
            .await?
            .expect("policy with version present");
        assert_eq!(found.version.id, created.version.id);
        Ok(())
    }

    #[tokio::test]
    async fn definition_edit_mints_a_new_version_and_repoints_current() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let created = conn
            .create_workspace_policy(
                NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id),
                vec![1],
                None,
            )
            .await?;

        let v2 = conn
            .create_policy_version(
                seeded.workspace_id,
                created.policy.id,
                seeded.account_id,
                vec![2],
                None,
            )
            .await?;
        assert_eq!(v2.version_number, 2);

        // The policy now points at v2, but v1 is frozen and still resolvable.
        let current = conn
            .find_policy_with_version(seeded.workspace_id, created.policy.id)
            .await?
            .expect("policy present");
        assert_eq!(current.version.id, v2.id);
        assert_eq!(current.version.definition, vec![2]);

        let v1 = conn
            .find_policy_version(seeded.workspace_id, created.version.id)
            .await?
            .expect("v1 still resolvable");
        assert_eq!(v1.definition, vec![1]);

        // A further edit continues the sequence: next_number is max + 1 under the
        // parent-policy lock, so versions stay dense and monotonic.
        let v3 = conn
            .create_policy_version(
                seeded.workspace_id,
                created.policy.id,
                seeded.account_id,
                vec![3],
                None,
            )
            .await?;
        assert_eq!(v3.version_number, 3);

        let versions = conn.list_policy_versions(created.policy.id).await?;
        assert_eq!(
            versions
                .iter()
                .map(|v| v.version_number)
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        Ok(())
    }

    #[tokio::test]
    async fn label_only_update_does_not_create_a_version() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let created = conn
            .create_workspace_policy(
                NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id),
                vec![1],
                None,
            )
            .await?;

        conn.update_workspace_policy(
            created.policy.id,
            UpdateWorkspacePolicy {
                display_name: Some("Renamed".to_owned()),
                ..Default::default()
            },
        )
        .await?;

        let versions = conn.list_policy_versions(created.policy.id).await?;
        assert_eq!(versions.len(), 1, "a label change mints no version");

        let current = conn
            .find_policy_with_version(seeded.workspace_id, created.policy.id)
            .await?
            .expect("policy present");
        assert_eq!(current.policy.display_name, "Renamed");
        assert_eq!(current.version.id, created.version.id);
        Ok(())
    }

    #[tokio::test]
    async fn versions_resolve_after_the_policy_is_soft_deleted() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let created = conn
            .create_workspace_policy(
                NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id),
                vec![1],
                None,
            )
            .await?;
        conn.delete_workspace_policy(created.policy.id).await?;

        // The logical policy is hidden, but its version (which a detection may have
        // pinned) still resolves by id.
        assert!(
            conn.find_policy_with_version(seeded.workspace_id, created.policy.id)
                .await?
                .is_none()
        );
        assert!(
            conn.find_policy_version(seeded.workspace_id, created.version.id)
                .await?
                .is_some()
        );
        Ok(())
    }
}
