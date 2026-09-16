//! Workspace repository for managing workspace operations.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspace, UpdateWorkspace, Workspace};
use crate::{Error, PgConnection, Result, schema};

/// Repository for workspace database operations.
///
/// Handles workspace lifecycle management: creation, lookup, updates, and
/// soft-deletion.
pub trait WorkspaceRepository {
    /// Creates a new workspace.
    ///
    /// Inserts a new workspace record with the provided configuration. A handle
    /// or display-name collision surfaces as a unique-constraint error for the
    /// caller to turn into a client error.
    fn create_workspace(
        &mut self,
        workspace: NewWorkspace,
    ) -> impl Future<Output = Result<Workspace>> + Send;

    /// Finds a workspace by ID, excluding soft-deleted workspaces.
    fn find_workspace_by_id(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = Result<Option<Workspace>>> + Send;

    /// Updates a workspace with partial changes.
    fn update_workspace(
        &mut self,
        workspace_id: Uuid,
        changes: UpdateWorkspace,
    ) -> impl Future<Output = Result<Workspace>> + Send;

    /// Soft deletes a workspace by setting the deletion timestamp.
    fn delete_workspace(&mut self, workspace_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Lists up to `limit` workspace ids that have been soft-deleted for at least
    /// `grace`, oldest deletion first — the workspaces the purge worker should
    /// reclaim (their grace window has elapsed).
    fn list_workspaces_pending_purge(
        &mut self,
        grace: std::time::Duration,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<Uuid>>> + Send;

    /// Tears down a soft-deleted workspace's blob references and marks the freed
    /// blobs expired, so the reaper reclaims their bytes.
    ///
    /// Soft-deletes the workspace's blob-holding entities (documents, detections,
    /// redactions), drops the reference each held — the same reference-drop the
    /// per-entity delete paths do, in bulk — and stamps `expires_at = now()` on
    /// those blobs. The expiry stamp is the one deliberate exception to "the
    /// workspace never touches blob state": once the grace window is up and the
    /// workspace is being destroyed, its data's retention no longer applies, and a
    /// blob kept indefinitely (`expires_at IS NULL`) would otherwise never become
    /// reclaimable and strand the workspace forever. It sets only the retention
    /// *flag*, never an object — the reaper still does all object deletion. Returns
    /// how many references were dropped. Idempotent: an already-soft-deleted entity
    /// is skipped, so it never double-drops.
    ///
    /// Call only for a workspace past its grace window (recoverability during the
    /// grace window depends on this not running before then).
    fn release_and_expire_workspace_blobs(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = Result<usize>> + Send;

    /// Whether a soft-deleted workspace's blobs are all gone (reclaimed by the
    /// reaper), so its rows can be hard-deleted without stranding any object.
    /// `true` when the workspace has no live blob row left. A read only — the
    /// workspace never mutates blob state.
    fn workspace_blobs_reclaimed(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Hard-deletes a soft-deleted workspace, cascading every child row away. Call
    /// only once [`workspace_blobs_reclaimed`](Self::workspace_blobs_reclaimed) is
    /// `true`, so the cascade removes the (already object-reclaimed) blob rows
    /// without leaving an object orphaned. Guarded on `deleted_at IS NOT NULL` so
    /// it never hard-deletes a live workspace.
    fn purge_workspace(&mut self, workspace_id: Uuid) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceRepository for PgConnection {
    async fn create_workspace(&mut self, workspace: NewWorkspace) -> Result<Workspace> {
        use schema::workspaces;

        let workspace = diesel::insert_into(workspaces::table)
            .values(&workspace)
            .returning(Workspace::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(workspace)
    }

    async fn find_workspace_by_id(&mut self, workspace_id: Uuid) -> Result<Option<Workspace>> {
        use schema::workspaces::{self, dsl};

        let workspace = workspaces::table
            .filter(dsl::id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(Workspace::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(workspace)
    }

    async fn update_workspace(
        &mut self,
        workspace_id: Uuid,
        changes: UpdateWorkspace,
    ) -> Result<Workspace> {
        use schema::workspaces::{self, dsl};

        let workspace = diesel::update(workspaces::table)
            .filter(dsl::id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .set(&changes)
            .returning(Workspace::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(workspace)
    }

    async fn delete_workspace(&mut self, workspace_id: Uuid) -> Result<()> {
        use schema::workspaces::dsl::{deleted_at, id, workspaces};

        diesel::update(workspaces)
            .filter(id.eq(workspace_id))
            .filter(deleted_at.is_null())
            .set(deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn list_workspaces_pending_purge(
        &mut self,
        grace: std::time::Duration,
        limit: i64,
    ) -> Result<Vec<Uuid>> {
        use schema::workspaces::{self, dsl};

        // A workspace is due for purge once it has been soft-deleted for at least
        // the grace window. The cutoff is computed here and compared against
        // `deleted_at` so the window is a plain timestamp predicate.
        let cutoff = jiff::Timestamp::now()
            - jiff::Span::try_from(grace)
                .map_err(|err| Error::unexpected(format!("invalid purge grace duration: {err}")))?;

        workspaces::table
            .filter(dsl::deleted_at.is_not_null())
            .filter(dsl::deleted_at.le(jiff_diesel::Timestamp::from(cutoff)))
            .order(dsl::deleted_at.asc())
            .limit(limit)
            .select(dsl::id)
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn release_and_expire_workspace_blobs(&mut self, workspace_id: Uuid) -> Result<usize> {
        use diesel_async::AsyncConnection;
        use schema::workspace_blobs::dsl as blobs;
        use schema::{
            workspace_blobs, workspace_detections, workspace_documents, workspace_redactions,
        };

        // Soft-delete the workspace's blob-holding entities, drop the reference each
        // still holds, and expire the freed blobs so the reaper reclaims their
        // bytes. Runs in one transaction so it all commits together.
        //
        // Blob references live on documents (`blob_id`), detections
        // (`intermediate_blob_id` and `audit_blob_id`), and redactions
        // (`review_audit_blob_id`). Each `UPDATE ... WHERE deleted_at IS NULL` is
        // idempotent (a second pass matches no live row), so a reference is never
        // double-dropped.
        self.transaction(async |conn| {
            let mut refs: Vec<Uuid> = Vec::new();

            let doc_blobs: Vec<Option<Uuid>> = diesel::update(
                workspace_documents::table
                    .filter(workspace_documents::workspace_id.eq(workspace_id))
                    .filter(workspace_documents::deleted_at.is_null()),
            )
            .set(workspace_documents::deleted_at.eq(now))
            .returning(workspace_documents::blob_id)
            .get_results(conn)
            .await
            .map_err(Error::from)?;
            refs.extend(doc_blobs.into_iter().flatten());

            // A detection holds two references (its intermediate and its base
            // analysis), returned together as it is soft-deleted.
            let detection_blobs: Vec<(Option<Uuid>, Option<Uuid>)> = diesel::update(
                workspace_detections::table
                    .filter(workspace_detections::workspace_id.eq(workspace_id))
                    .filter(workspace_detections::deleted_at.is_null()),
            )
            .set(workspace_detections::deleted_at.eq(now))
            .returning((
                workspace_detections::intermediate_blob_id,
                workspace_detections::audit_blob_id,
            ))
            .get_results(conn)
            .await
            .map_err(Error::from)?;
            for (intermediate, audit) in detection_blobs {
                refs.extend(intermediate);
                refs.extend(audit);
            }

            // Redactions scope to the workspace through their detection; each holds
            // its review-analysis reference.
            let review_blobs: Vec<Option<Uuid>> = diesel::update(
                workspace_redactions::table
                    .filter(workspace_redactions::deleted_at.is_null())
                    .filter(
                        workspace_redactions::detection_id.eq_any(
                            workspace_detections::table
                                .filter(workspace_detections::workspace_id.eq(workspace_id))
                                .select(workspace_detections::id),
                        ),
                    ),
            )
            .set(workspace_redactions::deleted_at.eq(now))
            .returning(workspace_redactions::review_audit_blob_id)
            .get_results(conn)
            .await
            .map_err(Error::from)?;
            refs.extend(review_blobs.into_iter().flatten());

            for blob_id in &refs {
                diesel::update(
                    workspace_blobs::table
                        .filter(blobs::id.eq(blob_id))
                        .filter(blobs::ref_count.gt(0)),
                )
                .set(blobs::ref_count.eq(blobs::ref_count - 1))
                .execute(conn)
                .await
                .map_err(Error::from)?;
            }

            // Expire the freed blobs so the reaper reclaims their bytes even when
            // retention was indefinite (`expires_at IS NULL`): the workspace is
            // being destroyed, so its data's retention no longer applies. This sets
            // only the retention flag; the reaper still does all object deletion.
            // Kept at or before the existing window so a blob due sooner is never
            // pushed later. A blob already claimed for purge (`purged_at`) is left
            // to the reaper.
            diesel::update(
                workspace_blobs::table
                    .filter(blobs::id.eq_any(&refs))
                    .filter(blobs::purged_at.is_null())
                    .filter(blobs::expires_at.is_null().or(blobs::expires_at.gt(now))),
            )
            .set(blobs::expires_at.eq(now))
            .execute(conn)
            .await
            .map_err(Error::from)?;

            Ok(refs.len())
        })
        .await
    }

    async fn workspace_blobs_reclaimed(&mut self, workspace_id: Uuid) -> Result<bool> {
        use schema::workspace_blobs::{self, dsl};

        // A blob row survives until the reaper has deleted its object and pruned it;
        // the workspace is ready to hard-delete only when none of its blobs remain.
        let remaining: i64 = workspace_blobs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::reclaimed_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(remaining == 0)
    }

    async fn purge_workspace(&mut self, workspace_id: Uuid) -> Result<()> {
        use schema::workspaces::{self, dsl};

        // Hard-delete the soft-deleted workspace; the FK cascade removes every child
        // row, including the (already object-reclaimed) blob rows. Guarded on
        // `deleted_at IS NOT NULL` so a live workspace is never purged.
        diesel::delete(
            workspaces::table
                .filter(dsl::id.eq(workspace_id))
                .filter(dsl::deleted_at.is_not_null()),
        )
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{NewWorkspace, UpdateWorkspace};
    use crate::query::WorkspaceRepository;
    use crate::test_util::TestDatabase;

    #[tokio::test]
    async fn create_then_find_by_id_round_trip() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let created = conn
            .create_workspace(NewWorkspace::test(account_id))
            .await?;

        // By id.
        let by_id = conn.find_workspace_by_id(created.id).await?;
        assert_eq!(by_id.map(|w| w.id), Some(created.id));
        Ok(())
    }

    #[tokio::test]
    async fn find_by_id_returns_none_for_missing_or_deleted() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        // A random id is not found.
        assert!(
            conn.find_workspace_by_id(uuid::Uuid::now_v7())
                .await?
                .is_none()
        );

        // A soft-deleted workspace is excluded.
        let ws = conn
            .create_workspace(NewWorkspace::test(account_id))
            .await?;
        conn.delete_workspace(ws.id).await?;
        assert!(conn.find_workspace_by_id(ws.id).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn update_applies_changes_and_skips_deleted_rows() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let ws = conn
            .create_workspace(NewWorkspace::test(account_id))
            .await?;

        let updated = conn
            .update_workspace(
                ws.id,
                UpdateWorkspace {
                    display_name: Some("Renamed".to_owned()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(updated.display_name, "Renamed");

        // Updating a soft-deleted workspace matches no live row and errors.
        conn.delete_workspace(ws.id).await?;
        let after_delete = conn
            .update_workspace(
                ws.id,
                UpdateWorkspace {
                    display_name: Some("Nope".to_owned()),
                    ..Default::default()
                },
            )
            .await;
        assert!(after_delete.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn purge_lifecycle_reclaims_blobs_then_hard_deletes_the_workspace() -> anyhow::Result<()>
    {
        use crate::query::{
            WorkspaceBlobRepository, WorkspaceDocumentRepository, WorkspaceRepository,
        };

        let db = TestDatabase::start().await;
        // A workspace with a document (and thus one live blob).
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let document = conn
            .find_document_in_workspace(seeded.workspace_id, seeded.document_id)
            .await?
            .expect("document present");
        let blob_id = document.blob_id.expect("document has a blob");

        // Soft-delete the workspace, then a fresh soft-delete is immediately "due"
        // with a zero grace window (the worker uses a real 30-day window).
        conn.delete_workspace(seeded.workspace_id).await?;
        let due = conn
            .list_workspaces_pending_purge(std::time::Duration::ZERO, 50)
            .await?;
        assert!(due.contains(&seeded.workspace_id), "the workspace is due");

        // The seeded blob has indefinite retention (`expires_at IS NULL`), so
        // without the purge's expire step it would never become reclaimable.
        assert!(
            conn.find_blob_by_id(blob_id)
                .await?
                .unwrap()
                .expires_at
                .is_none(),
            "the blob starts with indefinite retention"
        );

        // Phase 1: release the references the workspace's entities hold and expire
        // the freed blobs. The document is soft-deleted, its blob reference dropped
        // (ref_count -> 0), and the blob marked expired — so an indefinitely-kept
        // blob still becomes reclaimable. The blob row still exists, so the
        // workspace is NOT yet ready to hard-delete.
        let released = conn
            .release_and_expire_workspace_blobs(seeded.workspace_id)
            .await?;
        assert_eq!(released, 1, "the document's one blob reference is dropped");
        assert!(
            conn.find_document_in_workspace(seeded.workspace_id, seeded.document_id)
                .await?
                .is_none(),
            "the document is soft-deleted by the release"
        );
        assert_eq!(
            conn.find_blob_by_id(blob_id).await?.unwrap().ref_count,
            0,
            "the blob reference was released"
        );
        // The deadlock fix: the freed blob is now due for purge despite its
        // originally-indefinite retention.
        assert!(
            conn.blobs_due_for_purge(50)
                .await?
                .iter()
                .any(|b| b.id == blob_id),
            "the expired, unreferenced blob is now reclaimable"
        );
        assert!(
            !conn.workspace_blobs_reclaimed(seeded.workspace_id).await?,
            "not ready while the blob row still exists"
        );

        // Phase 2: the reaper reclaims the object (simulated here by claiming and
        // marking it reclaimed — the workspace never touches blob state itself).
        conn.claim_blob_for_purge(blob_id).await?;
        conn.mark_blob_reclaimed(blob_id).await?;
        assert!(
            conn.workspace_blobs_reclaimed(seeded.workspace_id).await?,
            "ready once every object is reclaimed"
        );

        // Phase 3: hard-delete the workspace; the cascade removes every child row.
        conn.purge_workspace(seeded.workspace_id).await?;
        assert!(
            conn.find_document_in_workspace(seeded.workspace_id, seeded.document_id)
                .await?
                .is_none(),
            "the document row is gone"
        );
        assert!(
            conn.find_blob_by_id(blob_id).await?.is_none(),
            "the blob row is cascade-deleted"
        );
        // No longer listed as pending (the row is gone).
        let after = conn
            .list_workspaces_pending_purge(std::time::Duration::ZERO, 50)
            .await?;
        assert!(!after.contains(&seeded.workspace_id));
        Ok(())
    }

    #[tokio::test]
    async fn purge_workspace_leaves_a_live_workspace_untouched() -> anyhow::Result<()> {
        use crate::query::WorkspaceRepository;

        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        // A live (not soft-deleted) workspace is guarded against purge.
        let ws = conn
            .create_workspace(NewWorkspace::test(account_id))
            .await?;
        conn.purge_workspace(ws.id).await?;
        assert!(
            conn.find_workspace_by_id(ws.id).await?.is_some(),
            "a live workspace is never hard-deleted by purge"
        );
        Ok(())
    }
}
