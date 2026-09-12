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
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, PolicyKind, WithAccountRef, keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// A logical policy paired with its current version's definition.
#[derive(Debug, Clone)]
pub struct PolicyWithVersion {
    /// The logical policy.
    pub policy: WorkspacePolicy,
    /// The live version whose definition the engine consumes.
    pub version: WorkspacePolicyVersion,
}

/// A resolved one-shot policy, and whether this call created it.
#[derive(Debug, Clone)]
pub struct OneshotPolicy {
    /// The policy and its current version.
    pub policy: PolicyWithVersion,
    /// `true` when a fresh row was inserted, `false` when an identical live
    /// one-shot was reused.
    pub created: bool,
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
        definition: serde_json::Value,
        version_metadata: Option<serde_json::Value>,
    ) -> impl Future<Output = Result<PolicyWithVersion>> + Send;

    /// Returns the live one-shot policy with the same content in this workspace,
    /// creating it if absent.
    ///
    /// One-shot policies are content-addressed by `content_hash` (a hash of the
    /// labels plus action): an identical one-shot is reused rather than duplicated,
    /// so re-redacting the same label set does not accumulate rows. A fresh policy
    /// is created with its first version; a reused one keeps its existing current
    /// version (no new version is minted). `new_policy.kind` must be `Oneshot` and
    /// `new_policy.content_hash` must match `content_hash`.
    fn find_or_create_oneshot_policy(
        &mut self,
        new_policy: NewWorkspacePolicy,
        content_hash: Vec<u8>,
        definition: serde_json::Value,
        version_metadata: Option<serde_json::Value>,
    ) -> impl Future<Output = Result<OneshotPolicy>> + Send;

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
                .create_workspace_policy(new_policy, serde_json::json!({ "test": true }), None)
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
        definition: serde_json::Value,
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

    /// Promotes a one-shot policy to authored, clearing its dedup hash, only while
    /// it is still a live one-shot. Returns whether the transition changed a row,
    /// so a concurrent promotion (or an already-authored policy) is a no-op rather
    /// than a duplicate.
    fn promote_policy_to_authored(
        &mut self,
        policy_id: Uuid,
    ) -> impl Future<Output = Result<bool>> + Send;

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
        definition: serde_json::Value,
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

    async fn find_or_create_oneshot_policy(
        &mut self,
        new_policy: NewWorkspacePolicy,
        content_hash: Vec<u8>,
        definition: serde_json::Value,
        version_metadata: Option<serde_json::Value>,
    ) -> Result<OneshotPolicy> {
        if new_policy.kind != PolicyKind::Oneshot
            || new_policy.content_hash.as_deref() != Some(content_hash.as_slice())
        {
            return Err(Error::unexpected(
                "one-shot policy kind and content hash do not match",
            ));
        }

        let workspace_id = new_policy.workspace_id;

        // Reuse an identical live one-shot if one already exists.
        if let Some(existing) = find_live_oneshot(self, workspace_id, &content_hash).await? {
            return Ok(OneshotPolicy {
                policy: existing,
                created: false,
            });
        }

        // Otherwise create it. A concurrent creator may have inserted the same
        // content between the lookup and here; the partial unique dedup index turns
        // that into a unique violation, at which point the existing row is read back.
        match self
            .create_workspace_policy(new_policy, definition, version_metadata)
            .await
        {
            Ok(policy) => Ok(OneshotPolicy {
                policy,
                created: true,
            }),
            Err(err) if err.is_unique_violation() => {
                let existing = find_live_oneshot(self, workspace_id, &content_hash)
                    .await?
                    .ok_or(err)?;
                Ok(OneshotPolicy {
                    policy: existing,
                    created: false,
                })
            }
            Err(err) => Err(err),
        }
    }

    async fn create_policy_version(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
        account_id: Uuid,
        definition: serde_json::Value,
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

        // One-shot policies are excluded: the default list shows only authored,
        // permanent policies. A promoted policy (now authored) appears here.
        let total = if pagination.include_count {
            Some(
                workspace_policies::table
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::kind.eq(PolicyKind::Authored))
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
            .filter(dsl::kind.eq(PolicyKind::Authored))
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

    async fn promote_policy_to_authored(&mut self, policy_id: Uuid) -> Result<bool> {
        use schema::workspace_policies::{self, dsl};

        // Conditional on the row still being a live one-shot, so the transition is
        // atomic: a concurrent promotion updates zero rows and the caller emits no
        // duplicate event.
        let affected = diesel::update(
            workspace_policies::table
                .filter(dsl::id.eq(policy_id))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::kind.eq(PolicyKind::Oneshot)),
        )
        .set((
            dsl::kind.eq(PolicyKind::Authored),
            dsl::content_hash.eq(None::<Vec<u8>>),
        ))
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(affected == 1)
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

/// Finds the live one-shot policy with `content_hash` in a workspace, paired with
/// its current version. Only `kind = Oneshot` rows carry a content hash, so this
/// never matches an authored policy.
async fn find_live_oneshot(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    content_hash: &[u8],
) -> Result<Option<PolicyWithVersion>> {
    use schema::workspace_policies::{self, dsl};

    let policy_id = workspace_policies::table
        .filter(dsl::workspace_id.eq(workspace_id))
        .filter(dsl::content_hash.eq(content_hash))
        .filter(dsl::deleted_at.is_null())
        .select(dsl::id)
        .first::<Uuid>(conn)
        .await
        .optional()
        .map_err(Error::from)?;

    match policy_id {
        Some(id) => conn.find_policy_with_version(workspace_id, id).await,
        None => Ok(None),
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
                serde_json::json!({ "v": 1 }),
                None,
            )
            .await?;
        assert_eq!(created.version.version_number, 1);
        assert_eq!(created.policy.current_version_id, Some(created.version.id));
        assert_eq!(created.version.definition, serde_json::json!({ "v": 1 }));

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
                serde_json::json!({ "v": 1 }),
                None,
            )
            .await?;

        let v2 = conn
            .create_policy_version(
                seeded.workspace_id,
                created.policy.id,
                seeded.account_id,
                serde_json::json!({ "v": 2 }),
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
        assert_eq!(current.version.definition, serde_json::json!({ "v": 2 }));

        let v1 = conn
            .find_policy_version(seeded.workspace_id, created.version.id)
            .await?
            .expect("v1 still resolvable");
        assert_eq!(v1.definition, serde_json::json!({ "v": 1 }));

        // A further edit continues the sequence: next_number is max + 1 under the
        // parent-policy lock, so versions stay dense and monotonic.
        let v3 = conn
            .create_policy_version(
                seeded.workspace_id,
                created.policy.id,
                seeded.account_id,
                serde_json::json!({ "v": 3 }),
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
                serde_json::json!({ "v": 1 }),
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
    async fn oneshot_policy_is_excluded_from_the_list_until_promoted() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let mut new_oneshot = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        new_oneshot.kind = PolicyKind::Oneshot;
        new_oneshot.content_hash = Some(vec![9, 9, 9]);
        let oneshot = conn
            .find_or_create_oneshot_policy(
                new_oneshot,
                vec![9, 9, 9],
                serde_json::json!({"v":1}),
                None,
            )
            .await?;
        assert!(oneshot.created);
        assert_eq!(oneshot.policy.policy.kind, PolicyKind::Oneshot);

        let permanent = conn
            .create_test_policy(NewWorkspacePolicy::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // The default list shows the authored policy but hides the one-shot.
        let page = conn
            .cursor_list_workspace_policies(seeded.workspace_id, CursorPagination::new(50))
            .await?;
        let listed: Vec<Uuid> = page.items.iter().map(|p| p.item.id).collect();
        assert!(listed.contains(&permanent.id));
        assert!(!listed.contains(&oneshot.policy.policy.id));

        // Promoting (authored, hash cleared) makes it appear.
        conn.update_workspace_policy(
            oneshot.policy.policy.id,
            UpdateWorkspacePolicy {
                kind: Some(PolicyKind::Authored),
                content_hash: Some(None),
                ..Default::default()
            },
        )
        .await?;
        let page = conn
            .cursor_list_workspace_policies(seeded.workspace_id, CursorPagination::new(50))
            .await?;
        assert!(
            page.items
                .iter()
                .any(|p| p.item.id == oneshot.policy.policy.id)
        );
        Ok(())
    }

    #[tokio::test]
    async fn oneshot_dedups_identical_content_and_distinguishes_different() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let mut first_new = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        first_new.kind = PolicyKind::Oneshot;
        first_new.content_hash = Some(vec![1, 1, 1]);
        let first = conn
            .find_or_create_oneshot_policy(
                first_new,
                vec![1, 1, 1],
                serde_json::json!({"v":7}),
                None,
            )
            .await?;
        assert!(first.created);

        // Same content reuses the same row and mints no new version.
        let mut again_new = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        again_new.kind = PolicyKind::Oneshot;
        again_new.content_hash = Some(vec![1, 1, 1]);
        let again = conn
            .find_or_create_oneshot_policy(
                again_new,
                vec![1, 1, 1],
                serde_json::json!({"v":7}),
                None,
            )
            .await?;
        assert!(!again.created, "identical content is reused");
        assert_eq!(again.policy.policy.id, first.policy.policy.id);
        assert_eq!(
            conn.list_policy_versions(first.policy.policy.id)
                .await?
                .len(),
            1,
            "reuse mints no new version"
        );

        // Different content creates a distinct row.
        let mut other_new = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        other_new.kind = PolicyKind::Oneshot;
        other_new.content_hash = Some(vec![2, 2, 2]);
        let other = conn
            .find_or_create_oneshot_policy(
                other_new,
                vec![2, 2, 2],
                serde_json::json!({"v":8}),
                None,
            )
            .await?;
        assert!(other.created);
        assert_ne!(other.policy.policy.id, first.policy.policy.id);
        Ok(())
    }

    #[tokio::test]
    async fn find_or_create_oneshot_rejects_mismatched_kind_or_hash() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // An authored kind is not a one-shot: rejected before any write.
        let mut wrong_kind = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        wrong_kind.kind = PolicyKind::Authored;
        wrong_kind.content_hash = Some(vec![9, 9, 9]);
        assert!(
            conn.find_or_create_oneshot_policy(
                wrong_kind,
                vec![9, 9, 9],
                serde_json::json!({"v":1}),
                None,
            )
            .await
            .is_err(),
            "authored kind is rejected"
        );

        // A content hash that disagrees with the lookup key is rejected.
        let mut wrong_hash = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        wrong_hash.kind = PolicyKind::Oneshot;
        wrong_hash.content_hash = Some(vec![1, 1, 1]);
        assert!(
            conn.find_or_create_oneshot_policy(
                wrong_hash,
                vec![2, 2, 2],
                serde_json::json!({"v":1}),
                None,
            )
            .await
            .is_err(),
            "mismatched content hash is rejected"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_promoted_oneshot_no_longer_dedups() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let mut new_oneshot = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        new_oneshot.kind = PolicyKind::Oneshot;
        new_oneshot.content_hash = Some(vec![5, 5, 5]);
        let oneshot = conn
            .find_or_create_oneshot_policy(
                new_oneshot,
                vec![5, 5, 5],
                serde_json::json!({"v":1}),
                None,
            )
            .await?;

        // Promotion clears the hash, so the content leaves the dedup set.
        conn.update_workspace_policy(
            oneshot.policy.policy.id,
            UpdateWorkspacePolicy {
                kind: Some(PolicyKind::Authored),
                content_hash: Some(None),
                ..Default::default()
            },
        )
        .await?;

        // A new one-shot with the same content now creates a fresh row.
        let mut again_new = NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        again_new.kind = PolicyKind::Oneshot;
        again_new.content_hash = Some(vec![5, 5, 5]);
        let again = conn
            .find_or_create_oneshot_policy(
                again_new,
                vec![5, 5, 5],
                serde_json::json!({"v":1}),
                None,
            )
            .await?;
        assert!(again.created);
        assert_ne!(again.policy.policy.id, oneshot.policy.policy.id);
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
                serde_json::json!({ "v": 1 }),
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

    #[tokio::test]
    async fn promote_to_authored_transitions_once() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let mut new_oneshot =
            NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id);
        new_oneshot.kind = PolicyKind::Oneshot;
        new_oneshot.content_hash = Some(vec![1, 2, 3]);
        let oneshot = conn
            .find_or_create_oneshot_policy(
                new_oneshot,
                vec![1, 2, 3],
                serde_json::json!({ "test": true }),
                None,
            )
            .await?;
        let policy_id = oneshot.policy.policy.id;

        // The first promotion flips the row; a second is a no-op, so a concurrent
        // caller cannot double-promote or emit a duplicate event.
        assert!(conn.promote_policy_to_authored(policy_id).await?);
        assert!(!conn.promote_policy_to_authored(policy_id).await?);

        let promoted = conn
            .find_policy_in_workspace(seeded.workspace_id, policy_id)
            .await?
            .expect("policy present");
        assert_eq!(promoted.kind, PolicyKind::Authored);
        assert!(promoted.content_hash.is_none());
        Ok(())
    }
}
