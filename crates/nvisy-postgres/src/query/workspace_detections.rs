//! Workspace detections repository for managing analysis instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    DetectionCallRecord, NewWorkspaceDetection, NewWorkspaceDetectionCall,
    NewWorkspaceDetectionCallMeter, UpdateWorkspaceDetection, WorkspaceDetection,
};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, DetectionFilter, DetectionStatus, Json, keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// The outcome of a guarded `edit_set` write.
#[derive(Debug, Clone)]
pub enum EditWriteOutcome {
    /// The write landed; carries the updated detection (with the bumped revision).
    Updated(Box<WorkspaceDetection>),
    /// The detection exists but its `edit_set_rev` no longer matches the expected
    /// one — a concurrent edit won, so this write was not applied.
    Stale,
    /// No live detection matches in the workspace.
    Absent,
}

/// Keyset for paginating detections: newest first by `started_at`, `id` as the
/// tiebreaker. Used by the workspace-scoped detection listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionCursor {
    /// When the detection started.
    pub started_at: Timestamp,
    /// Detection id (tiebreaker).
    pub id: uuid::Uuid,
}

/// Resolved display name of a detection's input document.
///
/// `None` when the document has been removed (e.g. by retention). Redacted
/// outputs belong to redactions, not the detection, so they are not resolved
/// here.
#[derive(Debug, Default, Clone)]
pub struct DetectionDocuments {
    /// Display name of the input document the detection analyzes.
    pub input: Option<String>,
}

/// One row of a detection listing: the detection plus the context a response
/// needs to render it without follow-up lookups — the triggering account and the
/// input document's display name (`None` if the document was removed).
#[derive(Debug, Clone)]
pub struct DetectionListRow {
    /// The detection.
    pub detection: WorkspaceDetection,
    /// The account that triggered the detection.
    pub account: AccountRefRow,
    /// Display name of the detection's input document, if still present.
    pub input_document_name: Option<String>,
}

/// Repository for workspace detection database operations.
///
/// Handles detection lifecycle management including creation, status updates,
/// completion tracking, and queries.
pub trait WorkspaceDetectionRepository {
    /// Creates a new workspace detection record.
    fn create_workspace_detection(
        &mut self,
        new_detection: NewWorkspaceDetection,
    ) -> impl Future<Output = Result<WorkspaceDetection>> + Send;

    /// Finds a detection by its opaque id, scoped to a workspace.
    ///
    /// The detection is addressed by its own id (behind `/detections/{detectionId}`)
    /// and scoped by its own `workspace_id`.
    fn find_workspace_detection_by_id(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceDetection>>> + Send;

    /// Finds a detection by its `(workspace, idempotency key)` pair, for detect
    /// replay.
    fn find_detection_by_idempotency_key(
        &mut self,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> impl Future<Output = Result<Option<WorkspaceDetection>>> + Send;

    /// Lists all of a workspace's detections with cursor pagination, scoped by the
    /// detection's own `workspace_id`. `filter` narrows by status, document, policy,
    /// and/or triggering account.
    fn cursor_list_workspace_detections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<DetectionCursor>,
        filter: &DetectionFilter,
    ) -> impl Future<Output = Result<CursorPage<DetectionListRow>>> + Send;

    /// Atomically claims a detection, transitioning it to `Executing`.
    ///
    /// Succeeds (returning the claimed detection) only when it is still `Pending`,
    /// or is `Executing` but its previous claim has gone stale (`claimed_at`
    /// older than `stale_before` — a worker that died mid-analysis). A detection
    /// already executing under a fresh claim, or past the detect phase, yields
    /// `None` so a redelivered job skips it instead of analyzing twice.
    ///
    /// The claim stamps `claimed_at = now()`, so the lease renews on each
    /// (re)claim. Callers pass `stale_before = now - lease` computed against the
    /// same clock the DB uses closely enough for a lease measured in minutes.
    fn claim_detection(
        &mut self,
        detection_id: Uuid,
        stale_before: jiff::Timestamp,
    ) -> impl Future<Output = Result<Option<WorkspaceDetection>>> + Send;

    /// Resolves the display name of a detection's input document.
    ///
    /// An indexed lookup by id; a document removed (e.g. by retention) yields
    /// `None`. Used to name a single detection's input document in its response
    /// without threading a join through the shared detection lookup.
    fn detection_document_names(
        &mut self,
        workspace_id: Uuid,
        detection: &WorkspaceDetection,
    ) -> impl Future<Output = Result<DetectionDocuments>> + Send;

    /// Transitions a detection to `Complete` only while the caller still holds
    /// its claim — the detection is still `Executing` and its `claimed_at` matches
    /// the value stamped when the caller claimed it. Returns `true` on success,
    /// `false` if the claim has gone stale (another worker re-claimed the
    /// detection after the lease expired), so the caller can abort without
    /// stamping over the new owner's work. `updates` carries the analyze results
    /// (audit blob, metadata); status and the claim guard are applied here.
    fn finalize_detection(
        &mut self,
        detection_id: Uuid,
        claimed_at: jiff::Timestamp,
        updates: UpdateWorkspaceDetection,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Transitions a detection to `Failed` only while the caller still holds its
    /// claim — the detection is still `Executing` and its `claimed_at` matches the
    /// value stamped when the caller claimed it. Returns `true` on success,
    /// `false` if the claim has gone stale (another worker re-claimed the
    /// detection). Mirrors [`finalize_detection`](Self::finalize_detection) for
    /// the failure path, so a worker whose lease expired mid-analysis cannot fail
    /// a detection another worker now owns. `updates` carries the failure reason
    /// and completion time; status and the claim guard are applied here.
    fn fail_detection(
        &mut self,
        detection_id: Uuid,
        claimed_at: jiff::Timestamp,
        updates: UpdateWorkspaceDetection,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Transitions a detection to `Failed` only while it is still `Pending` — no
    /// worker has claimed it. For the enqueue-failure path in the create handler:
    /// if enqueue reported an error but the job was in fact delivered, a worker
    /// may already have claimed the detection (moving it to `Executing`), and this
    /// guard makes the handler's failure a no-op so it never clobbers the outcome
    /// the worker will drive. Returns `true` if it failed the detection, `false`
    /// if it was no longer `Pending`. Forces `status` and `completed_at`.
    fn fail_pending_detection(
        &mut self,
        detection_id: Uuid,
        updates: UpdateWorkspaceDetection,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Sets a live detection's accumulated reviewer edits (`edit_set`), scoped to its
    /// workspace, under an optimistic-concurrency guard. `expected_rev` is the
    /// `edit_set_rev` the caller read; the write proceeds and bumps the revision only
    /// if it still matches. Pass `Some` to store the edits, `None` to clear them.
    ///
    /// Returns [`EditWriteOutcome::Updated`] with the new row, [`Stale`] when the
    /// detection exists but its revision has moved on (a concurrent edit won), or
    /// [`Absent`] when no live detection matches.
    ///
    /// [`Stale`]: EditWriteOutcome::Stale
    /// [`Absent`]: EditWriteOutcome::Absent
    fn set_detection_edits(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
        expected_rev: i32,
        edit_set: Option<Json<serde_json::Value>>,
    ) -> impl Future<Output = Result<EditWriteOutcome>> + Send;

    /// Records a detection's inference usage: one call row per model call, with
    /// the meters each spent. A no-op for an empty slice (a deterministic
    /// detection makes no model calls). Written once, at analyze time, inside the
    /// caller's finalize transaction.
    fn record_detection_usage(
        &mut self,
        calls: Vec<DetectionCallRecord>,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceDetectionRepository for PgConnection {
    async fn create_workspace_detection(
        &mut self,
        new_detection: NewWorkspaceDetection,
    ) -> Result<WorkspaceDetection> {
        use schema::workspace_detections;

        let detection = diesel::insert_into(workspace_detections::table)
            .values(&new_detection)
            .returning(WorkspaceDetection::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(detection)
    }

    async fn set_detection_edits(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
        expected_rev: i32,
        edit_set: Option<Json<serde_json::Value>>,
    ) -> Result<EditWriteOutcome> {
        use schema::workspace_detections::{self, dsl};

        // Update only when the revision still matches, bumping it so the next writer's
        // stale guard trips. The whole read-decide-write is one statement, so it is
        // atomic against a concurrent edit without a separate lock.
        let updated: Option<WorkspaceDetection> = diesel::update(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::edit_set_rev.eq(expected_rev)),
        )
        .set((
            dsl::edit_set.eq(edit_set),
            dsl::edit_set_rev.eq(dsl::edit_set_rev + 1),
        ))
        .returning(WorkspaceDetection::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(Error::from)?;

        if let Some(detection) = updated {
            return Ok(EditWriteOutcome::Updated(Box::new(detection)));
        }

        // No row updated: either the detection is gone (Absent) or its revision moved
        // on (Stale). A scoped existence check separates the two.
        let exists: bool = diesel::select(diesel::dsl::exists(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null()),
        ))
        .get_result(self)
        .await
        .map_err(Error::from)?;

        Ok(if exists {
            EditWriteOutcome::Stale
        } else {
            EditWriteOutcome::Absent
        })
    }

    async fn find_workspace_detection_by_id(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> Result<Option<WorkspaceDetection>> {
        use schema::workspace_detections;
        use schema::workspace_detections::dsl as detections;

        // Scope by the detection's own workspace.
        workspace_detections::table
            .filter(detections::id.eq(detection_id))
            .filter(detections::workspace_id.eq(workspace_id))
            .filter(detections::deleted_at.is_null())
            .select(WorkspaceDetection::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn find_detection_by_idempotency_key(
        &mut self,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<WorkspaceDetection>> {
        use schema::workspace_detections::{self, dsl};

        let detection = workspace_detections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::idempotency_key.eq(idempotency_key))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceDetection::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(detection)
    }

    async fn cursor_list_workspace_detections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<DetectionCursor>,
        filter: &DetectionFilter,
    ) -> Result<CursorPage<DetectionListRow>> {
        use schema::accounts::dsl as accounts;
        use schema::workspace_detections::dsl as detections;
        use schema::workspace_documents::dsl as documents;

        // Scope by the detection's own workspace. The triggering account and the
        // input document's name are selected alongside each detection so the
        // response can name them without a per-row lookup. The input document is
        // LEFT-joined so a document removed by retention yields a null name rather
        // than dropping the row.
        let scoped = || {
            let mut query = detections::workspace_detections
                .inner_join(accounts::accounts)
                .left_join(
                    documents::workspace_documents.on(detections::input_document_id
                        .eq(documents::id)
                        .and(documents::deleted_at.is_null())),
                )
                .filter(detections::workspace_id.eq(workspace_id))
                .filter(detections::deleted_at.is_null())
                .into_boxed();
            if let Some(status) = filter.status {
                query = query.filter(detections::status.eq(status));
            }
            if let Some(document_id) = filter.input_document_id {
                query = query.filter(detections::input_document_id.eq(document_id));
            }
            if let Some(account_id) = filter.account_id {
                query = query.filter(detections::account_id.eq(account_id));
            }
            if let Some(policy_id) = filter.policy_id {
                use schema::{
                    workspace_detection_policy_versions as dpv, workspace_policy_versions as pv,
                };
                // A detection ran against a policy when it pinned any version of
                // that policy. EXISTS avoids the row duplication a join would cause
                // for a detection that pinned several versions.
                query = query.filter(diesel::dsl::exists(
                    dpv::table.inner_join(pv::table).filter(
                        dpv::detection_id
                            .eq(detections::id)
                            .and(pv::policy_id.eq(policy_id)),
                    ),
                ));
            }
            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let selection = (
            WorkspaceDetection::as_select(),
            (
                accounts::id,
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
            documents::display_name.nullable(),
        );

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.started_at), k.id));
        let rows: Vec<(WorkspaceDetection, AccountRefRow, Option<String>)> = keyset!(
            scoped(),
            detections::started_at,
            detections::id,
            pagination.direction,
            after
        )
        .select(selection)
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        let items = rows
            .into_iter()
            .map(
                |(detection, account, input_document_name)| DetectionListRow {
                    detection,
                    account,
                    input_document_name,
                },
            )
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            DetectionCursor {
                started_at: row.detection.started_at.into(),
                id: row.detection.id,
            }
        }))
    }

    async fn claim_detection(
        &mut self,
        detection_id: Uuid,
        stale_before: jiff::Timestamp,
    ) -> Result<Option<WorkspaceDetection>> {
        use schema::workspace_detections::{self, dsl};

        let stale_before = jiff_diesel::Timestamp::from(stale_before);

        // Claim only if still pending, or executing under a claim that has gone
        // stale (a dead worker). The WHERE clause makes the transition atomic:
        // two concurrent deliveries race on the same row and exactly one flips
        // it to `executing`; the loser matches no row and gets `None`.
        let claimed = diesel::update(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(dsl::deleted_at.is_null())
                .filter(
                    dsl::status.eq(DetectionStatus::Pending).or(dsl::status
                        .eq(DetectionStatus::Executing)
                        .and(dsl::claimed_at.lt(stale_before))),
                ),
        )
        .set((
            dsl::status.eq(DetectionStatus::Executing),
            dsl::claimed_at.eq(diesel::dsl::now),
        ))
        .returning(WorkspaceDetection::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(Error::from)?;

        Ok(claimed)
    }

    async fn detection_document_names(
        &mut self,
        workspace_id: Uuid,
        detection: &WorkspaceDetection,
    ) -> Result<DetectionDocuments> {
        use schema::workspace_documents::{self, dsl};

        // Select the display name only, scoped to the workspace and excluding
        // soft-deleted documents, so a document removed by retention resolves to
        // `None`.
        async fn name_of(
            conn: &mut PgConnection,
            workspace_id: Uuid,
            document_id: Uuid,
        ) -> Result<Option<String>> {
            workspace_documents::table
                .filter(dsl::id.eq(document_id))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .select(dsl::display_name)
                .first::<String>(conn)
                .await
                .optional()
                .map_err(Error::from)
        }

        let input = name_of(self, workspace_id, detection.input_document_id).await?;

        Ok(DetectionDocuments { input })
    }

    async fn finalize_detection(
        &mut self,
        detection_id: Uuid,
        claimed_at: jiff::Timestamp,
        mut updates: UpdateWorkspaceDetection,
    ) -> Result<bool> {
        use schema::workspace_detections::{self, dsl};

        // Force the terminal transition and stamp the terminal time here, so every
        // terminal outcome records `completed_at` regardless of the caller; the
        // guard makes it a no-op unless we still own the claim.
        updates.status = Some(DetectionStatus::Complete);
        updates.completed_at = Some(Some(jiff::Timestamp::now().into()));
        let claimed_at = jiff_diesel::Timestamp::from(claimed_at);

        // Guard on the claim we hold: same detection, still `Executing`, and the
        // exact `claimed_at` our claim stamped. A worker that re-claimed a stale
        // detection renews `claimed_at`, so a lost claim matches no row and
        // returns false. The `deleted_at IS NULL` guard keeps a finalize that
        // raced a workspace purge from re-attaching blob pointers to the freed,
        // soft-deleted row (which later purge passes skip), stranding the blobs.
        let rows_affected = diesel::update(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::status.eq(DetectionStatus::Executing))
                .filter(dsl::claimed_at.eq(claimed_at)),
        )
        .set(&updates)
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(rows_affected == 1)
    }

    async fn fail_detection(
        &mut self,
        detection_id: Uuid,
        claimed_at: jiff::Timestamp,
        mut updates: UpdateWorkspaceDetection,
    ) -> Result<bool> {
        use schema::workspace_detections::{self, dsl};

        // Force the terminal transition and stamp the terminal time here, mirroring
        // `finalize_detection`, so both terminal outcomes record `completed_at`.
        updates.status = Some(DetectionStatus::Failed);
        updates.completed_at = Some(Some(jiff::Timestamp::now().into()));
        let claimed_at = jiff_diesel::Timestamp::from(claimed_at);

        // Same claim guard as the complete finalize, including `deleted_at IS
        // NULL`: only our still-live claim (detection `Executing`, `claimed_at`
        // unchanged, not purged) may fail the detection.
        let rows_affected = diesel::update(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::status.eq(DetectionStatus::Executing))
                .filter(dsl::claimed_at.eq(claimed_at)),
        )
        .set(&updates)
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(rows_affected == 1)
    }

    async fn fail_pending_detection(
        &mut self,
        detection_id: Uuid,
        mut updates: UpdateWorkspaceDetection,
    ) -> Result<bool> {
        use schema::workspace_detections::{self, dsl};

        // Force the terminal transition and stamp the terminal time, as the other
        // finalize methods do.
        updates.status = Some(DetectionStatus::Failed);
        updates.completed_at = Some(Some(jiff::Timestamp::now().into()));

        // Guard on `Pending`: once a worker claims the detection (moving it to
        // `Executing`), this matches no row and the worker owns the outcome. The
        // `deleted_at IS NULL` guard leaves a purged row to the reaper.
        let rows_affected = diesel::update(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::status.eq(DetectionStatus::Pending)),
        )
        .set(&updates)
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(rows_affected == 1)
    }

    async fn record_detection_usage(&mut self, calls: Vec<DetectionCallRecord>) -> Result<()> {
        use schema::{workspace_detection_call, workspace_detection_call_meter};

        if calls.is_empty() {
            return Ok(());
        }

        // Two bulk inserts, not a per-call loop: each call already carries its own
        // id (assigned at record construction), so every meter row can reference
        // its call without reading a generated key back. Flatten the records into
        // the call rows and the meter rows up front, then insert the calls and
        // their meters in one statement each. The caller runs this inside the
        // finalize transaction, so both inserts commit with the rest of finalize.
        let mut meter_rows: Vec<NewWorkspaceDetectionCallMeter> = Vec::new();
        let call_rows: Vec<NewWorkspaceDetectionCall> = calls
            .into_iter()
            .map(|DetectionCallRecord { call, meters }| {
                let call_id = call.id;
                meter_rows.extend(meters.into_iter().map(|(meter, amount)| {
                    NewWorkspaceDetectionCallMeter {
                        call_id,
                        meter,
                        amount,
                    }
                }));
                call
            })
            .collect();

        // Calls first so the meters' `call_id` foreign key resolves.
        diesel::insert_into(workspace_detection_call::table)
            .values(&call_rows)
            .execute(self)
            .await
            .map_err(Error::from)?;

        // A detection may make calls that billed nothing (e.g. failures), so the
        // meter set can be empty even when there are calls.
        if !meter_rows.is_empty() {
            diesel::insert_into(workspace_detection_call_meter::table)
                .values(&meter_rows)
                .execute(self)
                .await
                .map_err(Error::from)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};
    use uuid::Uuid;

    use super::*;
    use crate::model::{NewWorkspaceDetection, UpdateWorkspaceDetection};
    use crate::query::WorkspaceDetectionRepository;
    use crate::test_util::TestDatabase;
    use crate::types::DetectionStatus;

    #[tokio::test]
    async fn claim_transitions_pending_and_honors_a_fresh_lease() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        assert_eq!(detection.status, DetectionStatus::Pending);

        // A fresh claim succeeds and moves it to Executing with a lease stamp.
        let stale_before = Timestamp::now() - Span::new().minutes(5);
        let claimed = conn
            .claim_detection(detection.id, stale_before)
            .await?
            .expect("pending detection should claim");
        assert_eq!(claimed.status, DetectionStatus::Executing);
        assert!(claimed.claimed_at.is_some());

        // A redelivered job (claim still fresh) is skipped.
        assert!(
            conn.claim_detection(detection.id, stale_before)
                .await?
                .is_none()
        );

        // Once the lease is considered stale, it can be re-claimed (dead worker).
        let reclaimed = conn
            .claim_detection(detection.id, Timestamp::now() + Span::new().minutes(5))
            .await?;
        assert!(reclaimed.is_some(), "a stale claim should be re-claimable");
        Ok(())
    }

    #[tokio::test]
    async fn finalize_requires_holding_the_claim() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        let claimed = conn
            .claim_detection(detection.id, Timestamp::now() - Span::new().minutes(5))
            .await?
            .expect("claim");
        let claimed_at = jiff::Timestamp::from(claimed.claimed_at.expect("claimed_at set"));

        // Finalizing with a stale/wrong claim timestamp does nothing.
        let wrong = claimed_at - Span::new().minutes(1);
        assert!(
            !conn
                .finalize_detection(detection.id, wrong, UpdateWorkspaceDetection::default())
                .await?
        );

        // Finalizing with the exact claim we hold succeeds.
        assert!(
            conn.finalize_detection(
                detection.id,
                claimed_at,
                UpdateWorkspaceDetection::default()
            )
            .await?
        );
        let done = conn
            .find_workspace_detection_by_id(seeded.workspace_id, detection.id)
            .await?
            .expect("detection present");
        assert_eq!(done.status, DetectionStatus::Complete);
        assert!(done.completed_at.is_some());

        // Finalizing again is a no-op (no longer Executing).
        assert!(
            !conn
                .finalize_detection(
                    detection.id,
                    claimed_at,
                    UpdateWorkspaceDetection::default()
                )
                .await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn fail_detection_uses_the_same_claim_guard() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        let claimed = conn
            .claim_detection(detection.id, Timestamp::now() - Span::new().minutes(5))
            .await?
            .expect("claim");
        let claimed_at = jiff::Timestamp::from(claimed.claimed_at.expect("claimed_at set"));

        assert!(
            conn.fail_detection(
                detection.id,
                claimed_at,
                UpdateWorkspaceDetection::default()
            )
            .await?
        );
        let failed = conn
            .find_workspace_detection_by_id(seeded.workspace_id, detection.id)
            .await?
            .expect("present");
        assert_eq!(failed.status, DetectionStatus::Failed);
        assert!(failed.completed_at.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn fail_pending_detection_only_while_unclaimed() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        // A never-claimed detection can be failed by the enqueue-failure path.
        let pending = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        assert!(
            conn.fail_pending_detection(pending.id, UpdateWorkspaceDetection::default())
                .await?
        );

        // A claimed (Executing) detection is owned by its worker: the pending-fail
        // path is a no-op and does not clobber the outcome.
        let claimed_det = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        let _ = conn
            .claim_detection(claimed_det.id, Timestamp::now() - Span::new().minutes(5))
            .await?
            .expect("claim");
        assert!(
            !conn
                .fail_pending_detection(claimed_det.id, UpdateWorkspaceDetection::default())
                .await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn find_by_id_is_scoped_to_the_workspace() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;

        // Found within its own workspace.
        let found = conn
            .find_workspace_detection_by_id(seeded.workspace_id, detection.id)
            .await?
            .expect("detection present");
        assert_eq!(found.id, detection.id);

        // Not found scoped to another workspace.
        assert!(
            conn.find_workspace_detection_by_id(Uuid::now_v7(), detection.id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn idempotency_key_lookup_is_scoped_to_the_workspace() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let mut detection =
            NewWorkspaceDetection::test(seeded.workspace_id, seeded.account_id, seeded.document_id);
        detection.idempotency_key = Some("key-123".to_owned());
        let detection = conn.create_workspace_detection(detection).await?;

        let found = conn
            .find_detection_by_idempotency_key(seeded.workspace_id, "key-123")
            .await?;
        assert_eq!(found.map(|d| d.id), Some(detection.id));

        // A different key does not match.
        assert!(
            conn.find_detection_by_idempotency_key(seeded.workspace_id, "other")
                .await?
                .is_none()
        );

        // A different workspace does not match.
        assert!(
            conn.find_detection_by_idempotency_key(Uuid::now_v7(), "key-123")
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_filters_by_status_and_names_the_input_document() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        // A pending detection and a completed one on the same document.
        let pending = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        let to_complete = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        let claimed = conn
            .claim_detection(to_complete.id, Timestamp::now() - Span::new().minutes(5))
            .await?
            .expect("claim");
        let _ = conn
            .finalize_detection(
                to_complete.id,
                jiff::Timestamp::from(claimed.claimed_at.expect("claimed")),
                UpdateWorkspaceDetection::default(),
            )
            .await?;

        // Filter to Pending: only the pending detection, and the input document is
        // named.
        let page = conn
            .cursor_list_workspace_detections(
                seeded.workspace_id,
                CursorPagination::new(50),
                &DetectionFilter {
                    status: Some(DetectionStatus::Pending),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            page.items
                .iter()
                .map(|r| r.detection.id)
                .collect::<Vec<_>>(),
            vec![pending.id]
        );
        assert!(
            page.items[0].input_document_name.is_some(),
            "input document name should resolve through the join"
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_filters_by_policy() -> anyhow::Result<()> {
        use crate::model::NewWorkspacePolicy;
        use crate::query::{DetectionPolicyVersionRepository, WorkspacePolicyRepository};

        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        // Two policies, each with one version.
        let policy_a = conn
            .create_workspace_policy(
                NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id),
                serde_json::json!({ "v": "a" }),
                None,
            )
            .await?;
        let policy_b = conn
            .create_workspace_policy(
                NewWorkspacePolicy::test(seeded.workspace_id, seeded.account_id),
                serde_json::json!({ "v": "b" }),
                None,
            )
            .await?;

        // One detection pinned to policy A, another to policy B.
        let detection_a = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        conn.record_detection_policy_versions(detection_a.id, &[policy_a.version.id])
            .await?;
        let detection_b = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        conn.record_detection_policy_versions(detection_b.id, &[policy_b.version.id])
            .await?;

        // Filtering by policy A returns only the detection that ran against it.
        let page = conn
            .cursor_list_workspace_detections(
                seeded.workspace_id,
                CursorPagination::new(50),
                &DetectionFilter {
                    policy_id: Some(policy_a.policy.id),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            page.items
                .iter()
                .map(|r| r.detection.id)
                .collect::<Vec<_>>(),
            vec![detection_a.id]
        );
        Ok(())
    }

    #[tokio::test]
    async fn record_detection_usage_is_a_noop_for_no_calls() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let mut conn = db.client.get_connection().await?;

        // No calls writes nothing and does not error.
        conn.record_detection_usage(Vec::new()).await?;
        Ok(())
    }

    #[tokio::test]
    async fn expired_intermediate_is_nulled_and_its_blob_becomes_reclaimable() -> anyhow::Result<()>
    {
        use crate::model::NewBlob;
        use crate::query::WorkspaceBlobRepository;
        use crate::test_util::backdate;

        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;

        // Attach an intermediate blob the way the worker does: resolve the blob
        // (recording a reference), then finalize the claimed detection pointing at
        // it.
        let mut new_blob = NewBlob::test(seeded.workspace_id);
        new_blob.expires_at = Some((Timestamp::now() + Span::new().hours(1)).into());
        let blob = conn.find_or_create_blob(new_blob).await?;
        assert_eq!(blob.ref_count, 1);

        let claimed = conn
            .claim_detection(detection.id, Timestamp::now() - Span::new().minutes(5))
            .await?
            .expect("claim");
        let finalized = conn
            .finalize_detection(
                detection.id,
                jiff::Timestamp::from(claimed.claimed_at.expect("claimed")),
                UpdateWorkspaceDetection {
                    intermediate_blob_id: Some(Some(blob.id)),
                    ..Default::default()
                },
            )
            .await?;
        assert!(finalized);

        backdate::blob_span(
            &mut conn,
            blob.id,
            Timestamp::now() - Span::new().hours(2),
            Timestamp::now() - Span::new().hours(1),
        )
        .await?;

        // Pinned by the detection's reference, so not yet due.
        assert!(
            !conn
                .blobs_due_for_purge(50)
                .await?
                .iter()
                .any(|b| b.id == blob.id),
            "a referenced intermediate blob is not due while the pointer is set"
        );

        let cleared = crate::query::clear_expired_detection_intermediates(&mut conn, 50).await?;
        assert_eq!(cleared, 1);

        // The pointer is nulled and the blob dropped to zero references, so it is
        // now reclaimable; the detection row itself is retained.
        let after = conn
            .find_workspace_detection_by_id(seeded.workspace_id, detection.id)
            .await?
            .expect("detection retained");
        assert!(after.intermediate_blob_id.is_none());
        let blob_after = conn.find_blob_by_id(blob.id).await?.expect("blob present");
        assert_eq!(blob_after.ref_count, 0);
        assert!(
            conn.blobs_due_for_purge(50)
                .await?
                .iter()
                .any(|b| b.id == blob.id),
            "an unreferenced, expired intermediate blob is due for purge"
        );
        Ok(())
    }

    #[tokio::test]
    async fn set_detection_edits_is_guarded_by_revision() -> anyhow::Result<()> {
        use crate::query::EditWriteOutcome;
        use crate::types::Json;

        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;
        assert_eq!(detection.edit_set_rev, 0, "a new detection starts at rev 0");

        let edits = || Some(Json::from_raw(serde_json::json!({ "v": 1 })));

        // A write at the current revision lands and bumps the revision.
        let first = conn
            .set_detection_edits(seeded.workspace_id, detection.id, 0, edits())
            .await?;
        let EditWriteOutcome::Updated(updated) = first else {
            panic!("expected the write to land");
        };
        assert_eq!(updated.edit_set_rev, 1);

        // A second write at the now-stale revision 0 is rejected, nothing changes.
        let stale = conn
            .set_detection_edits(seeded.workspace_id, detection.id, 0, edits())
            .await?;
        assert!(matches!(stale, EditWriteOutcome::Stale));

        // A write at the current revision 1 lands again.
        let next = conn
            .set_detection_edits(seeded.workspace_id, detection.id, 1, None)
            .await?;
        let EditWriteOutcome::Updated(cleared) = next else {
            panic!("expected the write to land");
        };
        assert_eq!(cleared.edit_set_rev, 2);
        assert!(cleared.edit_set.is_none(), "None clears the edits");

        // A write against a detection that does not exist reports Absent.
        let absent = conn
            .set_detection_edits(seeded.workspace_id, uuid::Uuid::now_v7(), 0, edits())
            .await?;
        assert!(matches!(absent, EditWriteOutcome::Absent));
        Ok(())
    }
}
