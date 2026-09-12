//! Workspace policy version repository: reads over immutable policy snapshots.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::WorkspacePolicyVersion;
use crate::{Error, PgConnection, Result, schema};

/// Read operations on workspace policy versions.
pub trait WorkspacePolicyVersionRepository {
    /// Finds a policy version by id within a workspace.
    ///
    /// A version resolves even after its policy is soft-deleted, so a detection's
    /// pinned version can always be read back.
    fn find_policy_version(
        &mut self,
        workspace_id: Uuid,
        version_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspacePolicyVersion>>> + Send;

    /// Lists a policy's versions, newest first.
    fn list_policy_versions(
        &mut self,
        policy_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WorkspacePolicyVersion>>> + Send;
}

impl WorkspacePolicyVersionRepository for PgConnection {
    async fn find_policy_version(
        &mut self,
        workspace_id: Uuid,
        version_id: Uuid,
    ) -> Result<Option<WorkspacePolicyVersion>> {
        use schema::workspace_policy_versions::{self, dsl};

        workspace_policy_versions::table
            .filter(dsl::id.eq(version_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .select(WorkspacePolicyVersion::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_policy_versions(
        &mut self,
        policy_id: Uuid,
    ) -> Result<Vec<WorkspacePolicyVersion>> {
        use schema::workspace_policy_versions::{self, dsl};

        workspace_policy_versions::table
            .filter(dsl::policy_id.eq(policy_id))
            .order(dsl::version_number.desc())
            .select(WorkspacePolicyVersion::as_select())
            .load(self)
            .await
            .map_err(Error::from)
    }
}
