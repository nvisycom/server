//! Workspace audit repository: the engine's findings sets, with lineage.
//!
//! An audit's bytes live in a blob; the row records which detection it belongs to
//! and, for a review audit, which redaction produced it and which base audit it
//! was edited from. Creating an audit records a reference to its blob.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use super::workspace_blobs::WorkspaceBlobRepository;
use crate::model::{NewBlob, NewWorkspaceAudit, WorkspaceAudit};
use crate::{Error, PgConnection, Result, schema};

/// Read and write operations on workspace audits.
pub trait WorkspaceAuditRepository {
    /// Inserts an audit backed by `new_blob`, sharing an existing blob with
    /// identical content or inserting a fresh one, and records the audit's
    /// reference to it — all in one transaction so the audit and its blob
    /// reference commit together.
    ///
    /// `new_audit.blob_id` is ignored; the audit is pointed at the resolved blob.
    fn create_audit(
        &mut self,
        new_audit: NewWorkspaceAudit,
        new_blob: NewBlob,
    ) -> impl Future<Output = Result<WorkspaceAudit>> + Send;

    /// Finds a detection's base audit (the one with no redaction), if any.
    fn find_base_audit(
        &mut self,
        detection_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceAudit>>> + Send;

    /// Finds the audit a redaction produced, if any.
    fn find_redaction_audit(
        &mut self,
        redaction_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceAudit>>> + Send;

    /// Deletes up to `limit` audits whose blob has passed its retention window,
    /// dropping each deleted audit's reference to its blob, and returns how many
    /// were removed.
    ///
    /// Audit-log retention expires the finding record together with its bytes: an
    /// audit's blob carries the `AuditLogs` window, but nothing else releases the
    /// audit's reference, so the blob would stay pinned at `ref_count >= 1` and
    /// never reclaim. Releasing the reference here lets the blob reaper purge the
    /// bytes once `ref_count` reaches zero. Runs in one transaction so an audit and
    /// its reference drop commit together.
    fn delete_expired_audits(&mut self, limit: i64) -> impl Future<Output = Result<usize>> + Send;
}

impl WorkspaceAuditRepository for PgConnection {
    async fn create_audit(
        &mut self,
        mut new_audit: NewWorkspaceAudit,
        new_blob: NewBlob,
    ) -> Result<WorkspaceAudit> {
        use diesel_async::AsyncConnection;
        use schema::workspace_audits;

        self.transaction(async |conn| {
            // Resolve (share or insert) the blob, which records this audit's
            // reference; then insert the audit pointed at it. One reference is
            // recorded per referrer, in the transaction that creates the referrer.
            let blob = conn.find_or_create_blob(new_blob).await?;
            new_audit.blob_id = blob.id;

            let audit = diesel::insert_into(workspace_audits::table)
                .values(&new_audit)
                .returning(WorkspaceAudit::as_returning())
                .get_result(conn)
                .await
                .map_err(Error::from)?;

            Ok(audit)
        })
        .await
    }

    async fn find_base_audit(&mut self, detection_id: Uuid) -> Result<Option<WorkspaceAudit>> {
        use schema::workspace_audits::{self, dsl};

        workspace_audits::table
            .filter(dsl::detection_id.eq(detection_id))
            .filter(dsl::redaction_id.is_null())
            .select(WorkspaceAudit::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn find_redaction_audit(&mut self, redaction_id: Uuid) -> Result<Option<WorkspaceAudit>> {
        use schema::workspace_audits::{self, dsl};

        workspace_audits::table
            .filter(dsl::redaction_id.eq(redaction_id))
            .select(WorkspaceAudit::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn delete_expired_audits(&mut self, limit: i64) -> Result<usize> {
        use diesel_async::AsyncConnection;
        use schema::{workspace_audits, workspace_blobs};

        self.transaction(async |conn| {
            // The audits whose blob has passed its retention window, with the blob
            // each references so its reference can be dropped after deletion.
            let expired: Vec<(Uuid, Uuid)> = workspace_audits::table
                .inner_join(
                    workspace_blobs::table.on(workspace_audits::blob_id.eq(workspace_blobs::id)),
                )
                .filter(workspace_blobs::expires_at.is_not_null())
                .filter(workspace_blobs::expires_at.lt(now))
                .filter(workspace_blobs::purged_at.is_null())
                .limit(limit)
                .select((workspace_audits::id, workspace_audits::blob_id))
                .load(conn)
                .await
                .map_err(Error::from)?;

            for (audit_id, blob_id) in &expired {
                diesel::delete(workspace_audits::table.filter(workspace_audits::id.eq(audit_id)))
                    .execute(conn)
                    .await
                    .map_err(Error::from)?;
                // Each audit recorded one reference to its blob when created, so
                // one delete drops one reference — correct even when several audits
                // share a deduplicated blob.
                conn.decrement_ref(*blob_id).await?;
            }

            Ok(expired.len())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NewBlob, NewWorkspaceDetection, NewWorkspaceRedaction};
    use crate::query::{
        WorkspaceAuditRepository, WorkspaceBlobRepository, WorkspaceDetectionRepository,
        WorkspaceRedactionRepository,
    };
    use crate::test_util::TestDatabase;

    #[tokio::test]
    async fn base_and_review_audits_record_lineage_and_reference_blobs() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.pipeline_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;

        // A base audit: detection set, redaction/derived_from NULL. Creating it
        // resolves and references its blob.
        let base = conn
            .create_audit(
                NewWorkspaceAudit::base(seeded.workspace_id, Uuid::nil(), detection.id),
                NewBlob::test(seeded.workspace_id),
            )
            .await?;
        assert_eq!(base.detection_id, detection.id);
        assert!(base.redaction_id.is_none());
        assert!(base.derived_from.is_none());
        let base_blob = conn
            .find_blob_by_id(base.blob_id)
            .await?
            .expect("base blob present");
        assert_eq!(
            base_blob.ref_count, 1,
            "creating the audit records one reference"
        );

        // The base audit is found by its detection.
        let found = conn
            .find_base_audit(detection.id)
            .await?
            .expect("base found");
        assert_eq!(found.id, base.id);

        // A review audit: same detection, plus redaction and derived_from.
        let redaction = conn
            .create_redaction(NewWorkspaceRedaction::test(detection.id, seeded.account_id))
            .await?;
        let review = conn
            .create_audit(
                NewWorkspaceAudit::review(
                    seeded.workspace_id,
                    Uuid::nil(),
                    detection.id,
                    redaction.id,
                    base.id,
                ),
                NewBlob::test(seeded.workspace_id),
            )
            .await?;
        assert_eq!(review.redaction_id, Some(redaction.id));
        assert_eq!(review.derived_from, Some(base.id));

        // The review audit is found by its redaction; the base lookup still returns
        // the base (redaction_id IS NULL), not the review.
        let by_redaction = conn
            .find_redaction_audit(redaction.id)
            .await?
            .expect("review found");
        assert_eq!(by_redaction.id, review.id);
        assert_eq!(
            conn.find_base_audit(detection.id).await?.map(|a| a.id),
            Some(base.id),
            "the base lookup ignores the review audit"
        );
        Ok(())
    }

    #[tokio::test]
    async fn expired_audit_is_deleted_and_its_blob_becomes_reclaimable() -> anyhow::Result<()> {
        use jiff::{Span, Timestamp};

        use crate::test_util::backdate;

        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.pipeline_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;

        // An audit with a valid future window, then backdated past due.
        let mut new_blob = NewBlob::test(seeded.workspace_id);
        new_blob.expires_at = Some((Timestamp::now() + Span::new().hours(1)).into());
        let audit = conn
            .create_audit(
                NewWorkspaceAudit::base(seeded.workspace_id, Uuid::nil(), detection.id),
                new_blob,
            )
            .await?;
        backdate::blob_span(
            &mut conn,
            audit.blob_id,
            Timestamp::now() - Span::new().hours(2),
            Timestamp::now() - Span::new().hours(1),
        )
        .await?;

        // The blob is pinned by the audit's reference, so it is not yet due.
        assert!(
            !conn
                .blobs_due_for_purge(50)
                .await?
                .iter()
                .any(|b| b.id == audit.blob_id),
            "a referenced audit blob is not due while the audit exists"
        );

        let deleted = conn.delete_expired_audits(50).await?;
        assert_eq!(deleted, 1);

        // The audit row is gone and its blob dropped to zero references, so the
        // expired blob is now reclaimable by the reaper's Expire sweep.
        assert!(conn.find_base_audit(detection.id).await?.is_none());
        let blob = conn
            .find_blob_by_id(audit.blob_id)
            .await?
            .expect("blob row still present until purged");
        assert_eq!(blob.ref_count, 0);
        assert!(
            conn.blobs_due_for_purge(50)
                .await?
                .iter()
                .any(|b| b.id == audit.blob_id),
            "an unreferenced, expired audit blob is due for purge"
        );
        Ok(())
    }

    #[tokio::test]
    async fn non_expired_audit_is_left_untouched() -> anyhow::Result<()> {
        use jiff::{Span, Timestamp};

        use crate::test_util::backdate;

        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.pipeline_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;

        // A window that is set but still in the future.
        let mut new_blob = NewBlob::test(seeded.workspace_id);
        new_blob.expires_at = Some((Timestamp::now() + Span::new().hours(1)).into());
        let audit = conn
            .create_audit(
                NewWorkspaceAudit::base(seeded.workspace_id, Uuid::nil(), detection.id),
                new_blob,
            )
            .await?;
        backdate::blob_span(
            &mut conn,
            audit.blob_id,
            Timestamp::now() - Span::new().hours(1),
            Timestamp::now() + Span::new().hours(1),
        )
        .await?;

        let deleted = conn.delete_expired_audits(50).await?;
        assert_eq!(deleted, 0, "an audit whose blob has not expired is kept");
        assert!(conn.find_base_audit(detection.id).await?.is_some());
        let blob = conn
            .find_blob_by_id(audit.blob_id)
            .await?
            .expect("blob present");
        assert_eq!(blob.ref_count, 1, "its reference is intact");
        Ok(())
    }
}
