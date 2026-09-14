//! Workspace pipeline domain logic: create, read, list, update, and delete.
//!
//! Owns the pipeline rules — splitting a request into the stored config and its
//! relational policy references, resolving those references, and keeping the two
//! consistent — factored out of the handler so they can be reasoned about (and
//! tested) independently of the HTTP flow.

use nvisy_postgres::model::WorkspacePipeline;
use nvisy_postgres::query::{
    PipelineCursor, PipelineReferenceRepository, WorkspacePipelineRepository,
};
use nvisy_postgres::types::{CursorPage, CursorPagination, PipelineStatus, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::input::{CreatePipelineInput, PipelineReferences, UpdatePipelineInput};
use crate::domain::output::PipelineWithReferences;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event;
use crate::service::event::EventEmitter;

/// Tracing target for pipeline domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::pipeline";

/// Creates, reads, updates, and deletes workspace pipelines.
///
/// Holds the Postgres client and acquires its own connection per call, so each
/// method is a self-contained transaction. Resolved per request from
/// [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct WorkspacePipelineService {
    postgres: PgClient,
}

impl WorkspacePipelineService {
    /// Creates a [`WorkspacePipelineService`] over the given connection pool.
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// Creates a pipeline: the row, its policy references, and the creation event
    /// commit together. Returns the pipeline with the ids it now references.
    ///
    /// An unknown referenced policy rejects the whole request before any write.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        input: CreatePipelineInput,
    ) -> Result<PipelineWithReferences> {
        let mut conn = self.postgres.get_connection().await?;

        let (new_pipeline, references) = input
            .into_parts(origin.workspace_id, origin.account_id)
            .map_err(serialize_error)?;
        let policy_ids = resolve_references(&mut conn, origin.workspace_id, &references).await?;

        let pipeline = conn
            .transaction(async |conn| {
                let pipeline = conn.create_workspace_pipeline(new_pipeline).await?;
                conn.replace_workspace_pipeline_policies(
                    pipeline.workspace_id,
                    pipeline.id,
                    &policy_ids,
                )
                .await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::PipelineCreated(event::PipelineCreated {
                        pipeline_id: pipeline.id,
                    }),
                )
                .await?;
                Ok::<_, Error>(pipeline)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, pipeline_id = %pipeline.id, "Pipeline created");
        Ok(PipelineWithReferences {
            pipeline,
            policy_ids: references.policy_ids,
        })
    }

    /// Lists a workspace's pipelines, newest first, optionally filtered by status
    /// and a name search.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<PipelineCursor>,
        status: Option<PipelineStatus>,
        search: Option<&str>,
    ) -> Result<CursorPage<WithAccountRef<WorkspacePipeline>>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_pipelines(workspace_id, pagination, status, search)
            .await?)
    }

    /// Finds a pipeline by id with its creator and referenced policy ids, or a
    /// NotFound.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
    ) -> Result<(WithAccountRef<WorkspacePipeline>, Vec<Uuid>)> {
        let mut conn = self.postgres.get_connection().await?;
        let found = find_pipeline(&mut conn, workspace_id, pipeline_id).await?;
        let policy_ids = conn.list_pipeline_policy_ids(found.item.id).await?;
        Ok((found, policy_ids))
    }

    /// Updates a pipeline, returning it with its current policy references.
    ///
    /// Only provided fields change. Supplying a definition replaces the policy
    /// references too; omitting it leaves them untouched. The write and its event
    /// commit together. An unknown referenced policy rejects the request before any
    /// write.
    pub async fn update(
        &self,
        origin: event::EventOrigin<'_>,
        pipeline_id: Uuid,
        input: UpdatePipelineInput,
    ) -> Result<PipelineWithReferences> {
        let mut conn = self.postgres.get_connection().await?;
        let existing = find_pipeline(&mut conn, origin.workspace_id, pipeline_id)
            .await?
            .item;

        let (update_data, references) = input
            .into_parts(existing.metadata.or_default())
            .map_err(serialize_error)?;

        // Resolve any supplied references up front so an unknown handle rejects
        // before the write.
        let resolved = match &references {
            Some(references) => {
                Some(resolve_references(&mut conn, origin.workspace_id, references).await?)
            }
            None => None,
        };

        let pipeline = conn
            .transaction(async |conn| {
                let pipeline = conn
                    .update_workspace_pipeline(pipeline_id, update_data)
                    .await?;
                if let Some(policy_ids) = &resolved {
                    conn.replace_workspace_pipeline_policies(
                        pipeline.workspace_id,
                        pipeline.id,
                        policy_ids,
                    )
                    .await?;
                }
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::PipelineUpdated(event::PipelineUpdated { pipeline_id }),
                )
                .await?;
                Ok::<_, Error>(pipeline)
            })
            .await?;

        // A supplied definition wrote fresh references; otherwise the live ones
        // are read back so the response reflects the pipeline's current state.
        let policy_ids = match references {
            Some(references) => references.policy_ids,
            None => conn.list_pipeline_policy_ids(pipeline.id).await?,
        };

        tracing::info!(target: TRACING_TARGET, "Pipeline updated");
        Ok(PipelineWithReferences {
            pipeline,
            policy_ids,
        })
    }

    /// Soft-deletes a pipeline from its workspace, recording the event atomically.
    pub async fn delete(&self, origin: event::EventOrigin<'_>, pipeline_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        // Confirm the pipeline exists in the workspace before deleting.
        find_pipeline(&mut conn, origin.workspace_id, pipeline_id).await?;

        conn.transaction(async |conn| {
            conn.delete_workspace_pipeline(pipeline_id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::PipelineDeleted(event::PipelineDeleted { pipeline_id }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Pipeline deleted");
        Ok(())
    }
}

/// Finds a pipeline within a workspace by id, with its creator, or a NotFound.
async fn find_pipeline(
    conn: &mut PgConn,
    workspace_id: Uuid,
    pipeline_id: Uuid,
) -> Result<WithAccountRef<WorkspacePipeline>> {
    conn.find_pipeline_in_workspace_by_id(workspace_id, pipeline_id)
        .await?
        .ok_or_else(|| Error::not_found("pipeline"))
}

/// Validates a set of policy ids as live authored policies within a workspace,
/// rejecting the whole request with a NotFound if any id is unknown.
async fn resolve_references(
    conn: &mut PgConn,
    workspace_id: Uuid,
    references: &PipelineReferences,
) -> Result<Vec<Uuid>> {
    conn.validate_policy_ids(workspace_id, &references.policy_ids)
        .await?
        .ok_or_else(|| Error::not_found("policy"))
}

/// Maps a definition (de)serialization failure to an internal error.
///
/// A definition that will not round-trip is a server-side data problem, not a
/// client error.
fn serialize_error(error: serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Failed to process pipeline definition")
        .with_context(error.to_string())
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::test_util::TestDatabase;

    use super::*;
    use crate::domain::input::PipelineDefinitionInput;
    use crate::extract::SecurityContext;

    fn security() -> SecurityContext {
        SecurityContext::default()
    }

    fn create_request(policy_ids: Vec<Uuid>) -> CreatePipelineInput {
        CreatePipelineInput {
            display_name: "Test pipeline".to_owned(),
            description: None,
            definition: Some(PipelineDefinitionInput {
                default_scope: None,
                policy_ids,
            }),
            status: None,
            retention: None,
        }
    }

    #[tokio::test]
    async fn creates_a_pipeline_with_references() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let service = WorkspacePipelineService::new(db.client.clone());
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security(),
        };

        let created = service.create(origin, create_request(Vec::new())).await?;
        assert_eq!(created.pipeline.display_name, "Test pipeline");
        assert!(created.policy_ids.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn find_returns_the_pipeline_and_its_reference_ids() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let service = WorkspacePipelineService::new(db.client.clone());
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security(),
        };

        let created = service.create(origin, create_request(Vec::new())).await?;

        let (found, ids) = service
            .find(seeded.workspace_id, created.pipeline.id)
            .await?;
        assert_eq!(found.item.id, created.pipeline.id);
        assert!(ids.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn delete_soft_deletes_the_pipeline() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let service = WorkspacePipelineService::new(db.client.clone());
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let created = service.create(origin(), create_request(Vec::new())).await?;
        let pipeline_id = created.pipeline.id;
        service.delete(origin(), pipeline_id).await?;

        let missing = service.find(seeded.workspace_id, pipeline_id).await;
        assert!(missing.is_err());
        Ok(())
    }
}
