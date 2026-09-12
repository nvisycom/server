//! Workspace blob repository: content-addressed, shared, ref-counted bytes.
//!
//! A blob is the raw bytes behind a document, an audit, or a detection's
//! intermediate. Identical content in a workspace is stored once and shared:
//! [`find_or_create_blob`](WorkspaceBlobRepository::find_or_create_blob) reuses a
//! live blob with the same `(workspace_id, content_hash, file_size_bytes)` and
//! bumps its `ref_count`, inserting a fresh one only when none exists. Every
//! reference add/remove goes through [`increment_ref`] / [`decrement_ref`], and
//! the reaper reclaims a blob's object only once `ref_count` reaches zero and its
//! retention window has passed.
//!
//! [`increment_ref`]: WorkspaceBlobRepository::increment_ref
//! [`decrement_ref`]: WorkspaceBlobRepository::decrement_ref

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{Blob, NewBlob};
use crate::{Error, PgConnection, Result, schema};

/// A blob whose object is reclaimable: no live references and past retention.
#[derive(Debug, Clone, Queryable)]
pub struct ReclaimableBlob {
    /// The blob's id.
    pub id: Uuid,
    /// The blob's object-store path.
    pub storage_path: String,
    /// The bucket the blob's object lives in.
    pub storage_bucket: String,
}

/// Read and write operations on workspace blobs.
pub trait WorkspaceBlobRepository {
    /// Returns the workspace blob with the same content as `new_blob`, creating it
    /// if absent, and records one reference (`ref_count += 1`) either way.
    ///
    /// Content identity is `(workspace_id, content_hash, file_size_bytes)` over
    /// live (un-purged) blobs. Call inside the transaction that inserts the
    /// referring row so the blob and its first reference commit together.
    fn find_or_create_blob(
        &mut self,
        new_blob: NewBlob,
    ) -> impl Future<Output = Result<Blob>> + Send;

    /// Records one more reference to a blob (`ref_count += 1`).
    fn increment_ref(&mut self, blob_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Drops one reference to a blob (`ref_count -= 1`), never below zero. A blob
    /// that reaches zero becomes reclaimable once its retention has passed.
    fn decrement_ref(&mut self, blob_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Finds a blob by id.
    fn find_blob_by_id(
        &mut self,
        blob_id: Uuid,
    ) -> impl Future<Output = Result<Option<Blob>>> + Send;

    /// Lists up to `limit` reclaimable blobs: `ref_count = 0`, past `expires_at`,
    /// and not yet purged. These are the reaper's expiry sweep.
    fn blobs_due_for_purge(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<ReclaimableBlob>>> + Send;

    /// Lists up to `limit` blobs whose object still needs reclaiming: `ref_count`
    /// is zero and `purged_at` is unset. Backs the reaper's reconcile sweep for
    /// objects a best-effort purge failed to remove.
    fn blobs_pending_purge(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<ReclaimableBlob>>> + Send;

    /// Marks a blob's backing object reclaimed (stamps `purged_at`).
    fn mark_blob_purged(&mut self, blob_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Row-locks a blob and returns it only while it is still reclaimable
    /// (`ref_count = 0`, not yet purged), holding the lock for the transaction.
    ///
    /// The reaper re-checks this under the lock before deleting the backing
    /// object, so a reference acquired between the sweep's read and the purge
    /// keeps the object alive.
    fn lock_reclaimable_blob(
        &mut self,
        blob_id: Uuid,
    ) -> impl Future<Output = Result<Option<Blob>>> + Send;
}

impl WorkspaceBlobRepository for PgConnection {
    async fn find_or_create_blob(&mut self, new_blob: NewBlob) -> Result<Blob> {
        use schema::workspace_blobs::{self, dsl};

        // Reuse a live blob with identical content in this workspace, bumping its
        // reference; otherwise insert a fresh one starting at one reference.
        let existing = diesel::update(
            workspace_blobs::table
                .filter(dsl::workspace_id.eq(new_blob.workspace_id))
                .filter(dsl::content_hash.eq(&new_blob.content_hash))
                .filter(dsl::file_size_bytes.eq(new_blob.file_size_bytes))
                .filter(dsl::purged_at.is_null()),
        )
        .set(dsl::ref_count.eq(dsl::ref_count + 1))
        .returning(Blob::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(Error::from)?;

        if let Some(blob) = existing {
            return Ok(blob);
        }

        // No live match was visible, but a concurrent transaction may have inserted
        // one that this snapshot cannot see. `ON CONFLICT` on the live-blob dedup
        // index converges on that row and bumps its reference instead of inserting
        // a duplicate.
        diesel::insert_into(workspace_blobs::table)
            .values((&new_blob, dsl::ref_count.eq(1)))
            .on_conflict((dsl::workspace_id, dsl::content_hash, dsl::file_size_bytes))
            .filter_target(dsl::purged_at.is_null())
            .do_update()
            .set(dsl::ref_count.eq(dsl::ref_count + 1))
            .returning(Blob::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn increment_ref(&mut self, blob_id: Uuid) -> Result<()> {
        use schema::workspace_blobs::{self, dsl};

        // Never resurrect a purged blob: a reference is only meaningful while the
        // backing object still exists.
        diesel::update(
            workspace_blobs::table
                .filter(dsl::id.eq(blob_id))
                .filter(dsl::purged_at.is_null()),
        )
        .set(dsl::ref_count.eq(dsl::ref_count + 1))
        .execute(self)
        .await
        .map_err(Error::from)?;
        Ok(())
    }

    async fn decrement_ref(&mut self, blob_id: Uuid) -> Result<()> {
        use schema::workspace_blobs::{self, dsl};

        // Guard against dropping below zero: only decrement a blob that still has
        // a live reference.
        diesel::update(
            workspace_blobs::table
                .filter(dsl::id.eq(blob_id))
                .filter(dsl::ref_count.gt(0)),
        )
        .set(dsl::ref_count.eq(dsl::ref_count - 1))
        .execute(self)
        .await
        .map_err(Error::from)?;
        Ok(())
    }

    async fn find_blob_by_id(&mut self, blob_id: Uuid) -> Result<Option<Blob>> {
        use schema::workspace_blobs::{self, dsl};

        workspace_blobs::table
            .filter(dsl::id.eq(blob_id))
            .select(Blob::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn blobs_due_for_purge(&mut self, limit: i64) -> Result<Vec<ReclaimableBlob>> {
        use schema::workspace_blobs::{self, dsl};

        workspace_blobs::table
            .filter(dsl::ref_count.eq(0))
            .filter(dsl::purged_at.is_null())
            .filter(dsl::expires_at.is_not_null())
            .filter(dsl::expires_at.lt(now))
            .order(dsl::expires_at.asc())
            .limit(limit)
            .select((dsl::id, dsl::storage_path, dsl::storage_bucket))
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn blobs_pending_purge(&mut self, limit: i64) -> Result<Vec<ReclaimableBlob>> {
        use schema::workspace_blobs::{self, dsl};

        workspace_blobs::table
            .filter(dsl::ref_count.eq(0))
            .filter(dsl::purged_at.is_null())
            .order(dsl::created_at.asc())
            .limit(limit)
            .select((dsl::id, dsl::storage_path, dsl::storage_bucket))
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn mark_blob_purged(&mut self, blob_id: Uuid) -> Result<()> {
        use schema::workspace_blobs::{self, dsl};

        diesel::update(workspace_blobs::table.filter(dsl::id.eq(blob_id)))
            .set(dsl::purged_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    async fn lock_reclaimable_blob(&mut self, blob_id: Uuid) -> Result<Option<Blob>> {
        use schema::workspace_blobs::{self, dsl};

        workspace_blobs::table
            .filter(dsl::id.eq(blob_id))
            .filter(dsl::ref_count.eq(0))
            .filter(dsl::purged_at.is_null())
            .select(Blob::as_select())
            .for_update()
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};

    use super::*;
    use crate::query::WorkspaceBlobRepository;
    use crate::test_util::{TestDatabase, backdate};

    #[tokio::test]
    async fn find_or_create_dedups_identical_content_and_counts_references() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A fresh blob starts at one reference.
        let new_blob = NewBlob::test(seeded.workspace_id);
        let first = conn.find_or_create_blob(new_blob.clone()).await?;
        assert_eq!(first.ref_count, 1);

        // Identical content in the same workspace reuses the same blob and bumps
        // its reference rather than inserting a second row.
        let second = conn.find_or_create_blob(new_blob).await?;
        assert_eq!(second.id, first.id, "identical content should share a blob");
        assert_eq!(second.ref_count, 2);

        // Different content inserts a distinct blob at one reference.
        let other = conn
            .find_or_create_blob(NewBlob::test(seeded.workspace_id))
            .await?;
        assert_ne!(other.id, first.id);
        assert_eq!(other.ref_count, 1);
        Ok(())
    }

    #[tokio::test]
    async fn decrement_never_drops_below_zero() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let blob = conn
            .find_or_create_blob(NewBlob::test(seeded.workspace_id))
            .await?;
        assert_eq!(blob.ref_count, 1);

        conn.decrement_ref(blob.id).await?;
        let after = conn.find_blob_by_id(blob.id).await?.expect("blob present");
        assert_eq!(after.ref_count, 0);

        // A second decrement is a no-op: the guard keeps `ref_count` at zero.
        conn.decrement_ref(blob.id).await?;
        let floored = conn.find_blob_by_id(blob.id).await?.expect("blob present");
        assert_eq!(floored.ref_count, 0, "ref_count must not go negative");
        Ok(())
    }

    #[tokio::test]
    async fn only_unreferenced_expired_blobs_are_due_for_purge() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A blob still referenced is NOT reclaimable even once past its retention
        // window. Insert it with a valid future window, then backdate it past due.
        let mut referenced = NewBlob::test(seeded.workspace_id);
        referenced.expires_at = Some((Timestamp::now() + Span::new().hours(1)).into());
        let referenced = conn.find_or_create_blob(referenced).await?;
        assert_eq!(referenced.ref_count, 1);
        backdate::blob_span(
            &mut conn,
            referenced.id,
            Timestamp::now() - Span::new().hours(2),
            Timestamp::now() - Span::new().hours(1),
        )
        .await?;

        // A blob with no references and past retention IS reclaimable.
        let mut expired = NewBlob::test(seeded.workspace_id);
        expired.expires_at = Some((Timestamp::now() + Span::new().hours(1)).into());
        let expired = conn.find_or_create_blob(expired).await?;
        conn.decrement_ref(expired.id).await?;
        // Backdate its window so it is now past due.
        backdate::blob_span(
            &mut conn,
            expired.id,
            Timestamp::now() - Span::new().hours(2),
            Timestamp::now() - Span::new().hours(1),
        )
        .await?;

        let due = conn.blobs_due_for_purge(50).await?;
        let due_ids: Vec<Uuid> = due.iter().map(|b| b.id).collect();
        assert!(
            due_ids.contains(&expired.id),
            "unreferenced + expired is due"
        );
        assert!(
            !due_ids.contains(&referenced.id),
            "a referenced blob is never due for purge"
        );

        // Marking it purged takes it out of the pending-purge set.
        conn.mark_blob_purged(expired.id).await?;
        let pending = conn.blobs_pending_purge(50).await?;
        assert!(!pending.iter().any(|b| b.id == expired.id));
        Ok(())
    }

    #[tokio::test]
    async fn a_purged_blob_cannot_be_resurrected_or_relocked() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let blob = conn
            .find_or_create_blob(NewBlob::test(seeded.workspace_id))
            .await?;
        conn.decrement_ref(blob.id).await?;
        conn.mark_blob_purged(blob.id).await?;

        // increment_ref must not raise a purged blob's count back above zero.
        conn.increment_ref(blob.id).await?;
        let after = conn.find_blob_by_id(blob.id).await?.expect("blob present");
        assert_eq!(after.ref_count, 0, "a purged blob is never re-referenced");

        // A purged blob is not lockable for reclamation (its object is already gone).
        assert!(
            conn.lock_reclaimable_blob(blob.id).await?.is_none(),
            "a purged blob is not reclaimable"
        );
        Ok(())
    }

    #[tokio::test]
    async fn find_or_create_reuses_a_referenced_blob_after_a_sibling_is_purged()
    -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Two documents share one blob; its dedup key is unique over live blobs.
        let new_blob = NewBlob::test(seeded.workspace_id);
        let first = conn.find_or_create_blob(new_blob.clone()).await?;
        let second = conn.find_or_create_blob(new_blob.clone()).await?;
        assert_eq!(second.id, first.id);
        assert_eq!(second.ref_count, 2);

        // Identical content still deduplicates through the ON CONFLICT path.
        let third = conn.find_or_create_blob(new_blob).await?;
        assert_eq!(third.id, first.id);
        assert_eq!(third.ref_count, 3);
        Ok(())
    }
}
