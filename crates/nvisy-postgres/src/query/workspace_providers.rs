//! Workspace providers repository for managing encrypted inference providers.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceProvider, UpdateWorkspaceProvider, WorkspaceProvider};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, ProviderType, TimeCursor, WithAccountRef, keyset,
    total,
};
use crate::{PgConnection, Result, schema};

/// Repository for workspace inference-provider database operations.
///
/// Handles provider lifecycle management including creation, updates, and
/// workspace-scoped queries. Providers have no sync machinery.
pub trait WorkspaceProviderRepository {
    /// Creates a new workspace provider record.
    fn create_workspace_provider(
        &mut self,
        new_provider: NewWorkspaceProvider,
    ) -> impl Future<Output = Result<WorkspaceProvider>> + Send;

    /// Finds a provider by id within a specific workspace, with the handle and
    /// avatar of the account that created it. Excludes soft-deleted providers.
    fn find_provider_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        provider_id: Uuid,
    ) -> impl Future<Output = Result<Option<WithAccountRef<WorkspaceProvider>>>> + Send;

    /// Finds the workspace's most recently updated live, enabled provider of a
    /// given model type (e.g. its LLM), if any. Resolves a provider without
    /// decrypting every provider's config.
    ///
    /// Disabled (`is_active = false`) providers are excluded: a disabled provider
    /// is not usable, and a newer disabled one must not shadow an active one.
    fn find_provider_by_type(
        &mut self,
        workspace_id: Uuid,
        provider_type: ProviderType,
    ) -> impl Future<Output = Result<Option<WorkspaceProvider>>> + Send;

    /// Lists all providers in a workspace with cursor pagination, each paired with
    /// the handle and avatar of the account that created it.
    ///
    /// An empty `providers` slice means no provider filter; otherwise a provider
    /// matches if its concrete provider is any of the given ones.
    fn cursor_list_workspace_providers(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
        providers: &[String],
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceProvider>>>> + Send;

    /// Updates a provider's mutable fields.
    fn update_workspace_provider(
        &mut self,
        provider_id: Uuid,
        updates: UpdateWorkspaceProvider,
    ) -> impl Future<Output = Result<WorkspaceProvider>> + Send;

    /// Soft deletes a provider by setting the deletion timestamp.
    fn delete_workspace_provider(
        &mut self,
        provider_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceProviderRepository for PgConnection {
    async fn create_workspace_provider(
        &mut self,
        new_provider: NewWorkspaceProvider,
    ) -> Result<WorkspaceProvider> {
        use schema::workspace_providers;

        Ok(diesel::insert_into(workspace_providers::table)
            .values(&new_provider)
            .returning(WorkspaceProvider::as_returning())
            .get_result(self)
            .await?)
    }

    async fn find_provider_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        provider_id: Uuid,
    ) -> Result<Option<WithAccountRef<WorkspaceProvider>>> {
        use schema::workspace_providers::dsl;
        use schema::{accounts, workspace_providers};

        let row = workspace_providers::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::id.eq(provider_id))
            .filter(dsl::deleted_at.is_null())
            .select((WorkspaceProvider::as_select(), AccountRefRow::as_select()))
            .first::<(WorkspaceProvider, AccountRefRow)>(self)
            .await
            .optional()?;

        Ok(row.map(WithAccountRef::from))
    }

    async fn find_provider_by_type(
        &mut self,
        workspace_id: Uuid,
        provider_type: ProviderType,
    ) -> Result<Option<WorkspaceProvider>> {
        use schema::workspace_providers::{self, dsl};

        Ok(workspace_providers::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::provider_type.eq(provider_type))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::is_active.eq(true))
            // `id` breaks a tie so two providers updated in the same instant pick
            // a stable one rather than an arbitrary row per query.
            .order((dsl::updated_at.desc(), dsl::id.desc()))
            .select(WorkspaceProvider::as_select())
            .first(self)
            .await
            .optional()?)
    }

    async fn cursor_list_workspace_providers(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
        providers: &[String],
    ) -> Result<CursorPage<WithAccountRef<WorkspaceProvider>>> {
        use schema::workspace_providers::dsl;
        use schema::{accounts, workspace_providers};

        let scoped = || {
            let mut query = workspace_providers::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .into_boxed();
            if !providers.is_empty() {
                query = query.filter(dsl::provider.eq_any(providers));
            }
            query
        };
        let total = total!(pagination, self, scoped().count());
        let query = scoped().inner_join(accounts::table);

        let after = pagination.after_pair();
        let rows: Vec<(WorkspaceProvider, AccountRefRow)> =
            keyset!(query, dsl::created_at, dsl::id, pagination.direction, after)
                .select((WorkspaceProvider::as_select(), AccountRefRow::as_select()))
                .limit(pagination.fetch_limit())
                .load(self)
                .await?;

        let items: Vec<WithAccountRef<WorkspaceProvider>> =
            rows.into_iter().map(WithAccountRef::from).collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wp| {
            TimeCursor::new(wp.item.created_at, wp.item.id)
        }))
    }

    async fn update_workspace_provider(
        &mut self,
        provider_id: Uuid,
        updates: UpdateWorkspaceProvider,
    ) -> Result<WorkspaceProvider> {
        use schema::workspace_providers::{self, dsl};

        // Scope to a live row: a concurrent delete may have committed since the
        // caller's lookup, and updating the tombstoned row would revive it in
        // effect and emit a spurious event.
        Ok(diesel::update(
            workspace_providers::table
                .filter(dsl::id.eq(provider_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(&updates)
        .returning(WorkspaceProvider::as_returning())
        .get_result(self)
        .await?)
    }

    async fn delete_workspace_provider(&mut self, provider_id: Uuid) -> Result<()> {
        use diesel::dsl::now;
        use schema::workspace_providers::{self, dsl};

        // Scope to a live row so a concurrent delete is not overwritten with a
        // fresh `deleted_at`.
        diesel::update(
            workspace_providers::table
                .filter(dsl::id.eq(provider_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(now))
        .execute(self)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{NewWorkspaceProvider, UpdateWorkspaceProvider};
    use crate::query::WorkspaceProviderRepository;
    use crate::test_util::TestDatabase;
    use crate::types::{CursorPagination, ProviderType};

    #[tokio::test]
    async fn find_by_type_returns_the_most_recent_active_provider() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Two active LLM providers. `newer` is then updated, bumping its
        // `updated_at` past `older`'s, so the `updated_at DESC` ordering must
        // return `newer` — this is what exercises recency, not just presence.
        let _older = conn
            .create_workspace_provider(NewWorkspaceProvider::test(
                seeded.workspace_id,
                seeded.account_id,
                ProviderType::Llm,
            ))
            .await?;
        let newer = conn
            .create_workspace_provider(NewWorkspaceProvider::test(
                seeded.workspace_id,
                seeded.account_id,
                ProviderType::Llm,
            ))
            .await?;
        let newer = conn
            .update_workspace_provider(
                newer.id,
                UpdateWorkspaceProvider {
                    display_name: Some("Bumped".to_owned()),
                    ..Default::default()
                },
            )
            .await?;

        // A disabled provider must never be returned, even if it is newest.
        let disabled = NewWorkspaceProvider {
            is_active: Some(false),
            ..NewWorkspaceProvider::test(seeded.workspace_id, seeded.account_id, ProviderType::Llm)
        };
        let _ = conn.create_workspace_provider(disabled).await?;

        let found = conn
            .find_provider_by_type(seeded.workspace_id, ProviderType::Llm)
            .await?;
        assert_eq!(found.map(|p| p.id), Some(newer.id));

        // A different kind in the same workspace is not matched.
        assert!(
            conn.find_provider_by_type(seeded.workspace_id, ProviderType::Ner)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_and_delete_are_scoped_to_live_rows() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let provider = conn
            .create_workspace_provider(NewWorkspaceProvider::test(
                seeded.workspace_id,
                seeded.account_id,
                ProviderType::Llm,
            ))
            .await?;

        // Soft-delete it, then a second delete and an update both find no live row.
        conn.delete_workspace_provider(provider.id).await?;
        assert!(
            conn.find_provider_in_workspace_with_creator(seeded.workspace_id, provider.id)
                .await?
                .is_none()
        );
        // Updating the tombstoned row matches nothing, so it errors as not-found
        // rather than reviving it.
        let update = UpdateWorkspaceProvider {
            display_name: Some("revived?".to_owned()),
            ..Default::default()
        };
        assert!(
            conn.update_workspace_provider(provider.id, update)
                .await
                .is_err()
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_filters_by_provider_and_paginates() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        for _ in 0..3 {
            let _ = conn
                .create_workspace_provider(NewWorkspaceProvider::test(
                    seeded.workspace_id,
                    seeded.account_id,
                    ProviderType::Llm,
                ))
                .await?;
        }

        let page = conn
            .cursor_list_workspace_providers(seeded.workspace_id, CursorPagination::new(50), &[])
            .await?;
        assert_eq!(page.items.len(), 3);

        // A provider filter that matches nothing returns an empty page.
        let none = conn
            .cursor_list_workspace_providers(
                seeded.workspace_id,
                CursorPagination::new(50),
                &["anthropic".to_owned()],
            )
            .await?;
        assert_eq!(none.items, vec![]);
        Ok(())
    }
}
