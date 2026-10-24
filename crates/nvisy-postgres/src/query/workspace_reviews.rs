//! Workspace review repository. A review is a named discussion on a document with
//! an explicit approval lifecycle (0..N per document, concurrent per audience): an author,
//! a title, a stream of comments and timeline events, and a status (`draft` →
//! `in_review` → `changes_requested`/`partially_approved` → `approved`, reopen). It
//! is anchored to its document and points at the detection it is currently reviewing
//! and the redaction it approves (direct columns, not link tables). A review is
//! created explicitly (not auto-created by detection/redaction). Opening a review
//! records the `review.opened` event; renaming, repointing, assignment, and each
//! lifecycle decision (`submitted`, `changes_requested`, `approved`, `reopened`)
//! record their own event on the review's one timeline ([`WorkspaceReviewEvent`]),
//! which the reader interleaves with the comments.
//!
//! [`WorkspaceReviewEvent`]: crate::model::WorkspaceReviewEvent

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::workspace_review_events::record_event;
use crate::model::{
    NewReviewAssignee, NewWorkspaceReview, UpdateWorkspaceReview, WorkspaceReview,
    WorkspaceReviewEvent,
};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, DocumentReviewFilter, ReviewEventKind,
    ReviewStatus, keyset,
};
use crate::{AsyncConnection, Error, PgConnection, Result, schema};

/// Keyset for paginating reviews: newest first by `created_at`, `id` tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentReviewCursor {
    /// When the review was created.
    pub created_at: Timestamp,
    /// Review id (tiebreaker).
    pub id: uuid::Uuid,
}

/// Keyset for paginating a review's activity events by `created_at`, `id`
/// tiebreaker (direction is the caller's).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewEventCursor {
    /// When the event happened.
    pub created_at: Timestamp,
    /// Event id (tiebreaker).
    pub id: uuid::Uuid,
}

/// A review paired with its assignees' account references (empty when unassigned).
#[derive(Debug, Clone)]
pub struct WithReviewers<T> {
    /// The review.
    pub item: T,
    /// The assigned reviewers' account references (empty when unassigned).
    pub assignees: Vec<AccountRefRow>,
}

/// Read and write operations on document reviews.
pub trait WorkspaceReviewRepository {
    /// Opens a new review on a document at [`ReviewStatus::Draft`], recording the
    /// `review.opened` timeline event (whose target carries the detection id when it
    /// opens against one), in one transaction, and returns the review.
    /// `display_name` is the review's title. A document may have many concurrent
    /// reviews (one per audience).
    fn create_review(
        &mut self,
        new_review: NewWorkspaceReview,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Finds a live review by id within a workspace.
    fn find_review(
        &mut self,
        workspace_id: Uuid,
        review_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceReview>>> + Send;

    /// Finds a live review by id within a workspace, taking a row lock (`FOR
    /// UPDATE`) so a concurrent status change serializes behind this read.
    ///
    /// Call inside a transaction that then acts on the review's state (e.g. posting
    /// a comment only while it is not resolved): the lock makes the check and the
    /// write atomic, closing the read-then-write race the unlocked
    /// [`find_review`](Self::find_review) leaves open.
    fn lock_review_in_workspace(
        &mut self,
        workspace_id: Uuid,
        review_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceReview>>> + Send;

    /// Lists a document's reviews, newest first, each paired with its assignees.
    fn list_document_reviews(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WithReviewers<WorkspaceReview>>>> + Send;

    /// Lists a workspace's reviews (the review queue) with cursor pagination, each
    /// paired with its assignees' account references.
    fn cursor_list_reviews(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<DocumentReviewCursor>,
        filter: &DocumentReviewFilter,
    ) -> impl Future<Output = Result<CursorPage<WithReviewers<WorkspaceReview>>>> + Send;

    /// Sets a review's title, recording a `review.renamed` timeline event carrying
    /// the new name, in one transaction.
    fn rename_review(
        &mut self,
        review_id: Uuid,
        display_name: String,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Soft-deletes a review and all of its comments (its events are left in place,
    /// hidden with the review).
    fn delete_review(&mut self, review_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Points a review at a detection (its current subject), recording a
    /// `review.redetected` event when the target changes. Idempotent when the review
    /// already points at `detection_id`. Returns the review.
    fn point_detection(
        &mut self,
        review_id: Uuid,
        detection_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Assigns a reviewer to a review (idempotent). Records an `assigned` event on
    /// first assignment. Assignment is metadata only (who is responsible); it does
    /// not change the review's status. The [`AssignmentOutcome`] reports the review
    /// and whether anything changed (`false` when the reviewer was already assigned).
    fn add_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<AssignmentOutcome>> + Send;

    /// Removes a reviewer from a review. Records an `unassigned` event when a link
    /// existed. Does not change the review's status. The [`AssignmentOutcome`] reports
    /// the review and whether a link was actually removed (`false` when the reviewer
    /// was not assigned).
    /// Removes `account_id` from the review's assignees, recording an `unassigned`
    /// event when a row was removed. If the removed reviewer had approved, their
    /// approval is removed with the row, so the review's status is reconciled under
    /// the same lock: should the remaining approvals fall below `required_approvals`,
    /// an `approved` review is demoted (clearing its approved output) and a
    /// `partially_approved` review with no approvals left returns to `in_review`.
    fn remove_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
        required_approvals: u32,
    ) -> impl Future<Output = Result<AssignmentOutcome>> + Send;

    /// Lists a review's assignees' account references.
    fn list_review_assignees(
        &mut self,
        review_id: Uuid,
    ) -> impl Future<Output = Result<Vec<AccountRefRow>>> + Send;

    /// Submits (or resubmits) a review for review, moving it from `draft` or
    /// `changes_requested` to `in_review` and recording a `submitted` event. Locks
    /// the review first and re-checks the status under the lock: a review in any other
    /// state is returned unchanged (`changed = false`), so a
    /// concurrent submit that lost the race neither reverts an approval nor re-fires
    /// the event.
    fn submit_review(
        &mut self,
        review_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<StatusOutcome>> + Send;

    /// Requests changes on a review, moving it to `changes_requested`, clearing all
    /// accumulated approvals, and recording a `changes_requested` event. Locks the
    /// review first.
    fn request_changes(
        &mut self,
        review_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Stamps `actor`'s approval on their assignee row (idempotent per reviewer) and
    /// advances the status: once the count of approved assignees reaches
    /// `required_approvals` the review becomes `approved` (and `redaction_id` is set
    /// when `output` is given), otherwise `partially_approved`. Records an `approved`
    /// event on a new approval. Locks the review first. The caller must already be an
    /// assignee (the domain layer enforces this); an un-assigned actor stamps nothing
    /// and the [`ApprovalOutcome`] reports `recorded = false` with the unchanged count.
    fn approve_review(
        &mut self,
        review_id: Uuid,
        actor: Uuid,
        required_approvals: u32,
        output: Option<Uuid>,
    ) -> impl Future<Output = Result<ApprovalOutcome>> + Send;

    /// Reopens an approved review back to `in_review`, clearing its approvals and its
    /// approved `redaction_id`, and recording a `reopened` event. Locks the review
    /// first and re-checks the status under the lock: a review that is not approved is
    /// returned unchanged (`changed = false`), so a concurrent
    /// reopen that lost the race does not re-fire the event.
    fn reopen_review(
        &mut self,
        review_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<StatusOutcome>> + Send;

    /// Lists a review's activity events with cursor pagination, each paired with
    /// the actor's account reference (when the account still exists). Order follows
    /// `pagination.direction`; pass [`Direction::Ascending`] for a chronological
    /// (oldest-first) timeline.
    ///
    /// [`Direction::Ascending`]: crate::types::Direction::Ascending
    fn cursor_list_review_events(
        &mut self,
        review_id: Uuid,
        pagination: CursorPagination<ReviewEventCursor>,
    ) -> impl Future<Output = Result<CursorPage<WithActor<WorkspaceReviewEvent>>>> + Send;
}

/// The result of an assignee add/remove: the review and whether the operation
/// actually changed anything (`false` for an idempotent no-op — an already-assigned
/// add or a not-assigned remove). Adding never changes the status; removing an
/// assignee who had approved can demote a review that thereby falls below quorum.
#[derive(Debug, Clone)]
pub struct AssignmentOutcome {
    /// The review after the operation.
    pub review: WorkspaceReview,
    /// Whether a link was added or removed (vs an idempotent no-op).
    pub changed: bool,
}

/// The result of a status transition (submit, reopen): the review and whether the
/// status actually changed (`false` for a no-op, e.g. a concurrent request that lost
/// the race under the lock). The caller fires the lifecycle event only when `changed`.
#[derive(Debug, Clone)]
pub struct StatusOutcome {
    /// The review after the operation.
    pub review: WorkspaceReview,
    /// Whether the status transitioned (vs an unchanged no-op).
    pub changed: bool,
}

/// The result of an approval: the (possibly status-advanced) review, whether this
/// approval was newly recorded (`false` when the reviewer had already approved), and
/// how many distinct approvals the review now holds.
#[derive(Debug, Clone)]
pub struct ApprovalOutcome {
    /// The review after the approval.
    pub review: WorkspaceReview,
    /// Whether a new approval row was inserted (vs an idempotent re-approval).
    pub recorded: bool,
    /// The count of distinct approvals on the review after the operation.
    pub approvals: u32,
    /// `true` when the approval named a different output than the one already bound
    /// to the review, so nothing was stamped. The domain layer maps this to a 409.
    pub rebind_rejected: bool,
}

/// A review event paired with its actor's account reference (absent when the
/// account was removed).
#[derive(Debug, Clone)]
pub struct WithActor<T> {
    /// The event.
    pub item: T,
    /// The actor's account reference, or `None` when the account was removed.
    pub actor: Option<AccountRefRow>,
}

impl WorkspaceReviewRepository for PgConnection {
    async fn create_review(&mut self, new_review: NewWorkspaceReview) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = diesel::insert_into(schema::workspace_reviews::table)
                .values(&new_review)
                .returning(WorkspaceReview::as_returning())
                .get_result::<WorkspaceReview>(conn)
                .await
                .map_err(Error::from)?;

            // Record the review's opening as the first timeline event, so the stream
            // begins with an explicit `review.opened` entry. When the review opens
            // against a detection, its target carries that detection id — the
            // timeline shows the review's initial subject without a separate event.
            let opened_target = review
                .detection_id
                .map(|detection_id| serde_json::json!({ "detectionId": detection_id }));
            record_event(
                conn,
                &review,
                ReviewEventKind::Opened,
                review.author_account_id,
                opened_target,
            )
            .await?;

            Ok(review)
        })
        .await
    }

    async fn find_review(
        &mut self,
        workspace_id: Uuid,
        review_id: Uuid,
    ) -> Result<Option<WorkspaceReview>> {
        use schema::workspace_reviews::{self, dsl};

        workspace_reviews::table
            .filter(dsl::id.eq(review_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceReview::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn lock_review_in_workspace(
        &mut self,
        workspace_id: Uuid,
        review_id: Uuid,
    ) -> Result<Option<WorkspaceReview>> {
        use schema::workspace_reviews::{self, dsl};

        workspace_reviews::table
            .filter(dsl::id.eq(review_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceReview::as_select())
            .for_update()
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_document_reviews(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> Result<Vec<WithReviewers<WorkspaceReview>>> {
        use schema::workspace_reviews::{self, dsl};

        let reviews: Vec<WorkspaceReview> = workspace_reviews::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .order((dsl::created_at.desc(), dsl::id.desc()))
            .select(WorkspaceReview::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        attach_assignees(self, reviews).await
    }

    async fn cursor_list_reviews(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<DocumentReviewCursor>,
        filter: &DocumentReviewFilter,
    ) -> Result<CursorPage<WithReviewers<WorkspaceReview>>> {
        use schema::workspace_reviews::{self, dsl};

        // The assignee filter is a membership test against the link table, so it
        // is applied as an EXISTS subquery rather than a join.
        let scoped = || {
            let mut query = workspace_reviews::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .into_boxed();
            if let Some(document_id) = filter.document_id {
                query = query.filter(dsl::document_id.eq(document_id));
            }
            if let Some(author_account_id) = filter.author_account_id {
                query = query.filter(dsl::author_account_id.eq(author_account_id));
            }
            if let Some(assignee) = filter.assignee_account_id {
                use schema::workspace_review_assignees as wra;
                query = query.filter(diesel::dsl::exists(
                    wra::table
                        .filter(wra::review_id.eq(dsl::id))
                        .filter(wra::account_id.eq(assignee)),
                ));
            }
            if let Some(review_status) = filter.review_status {
                query = query.filter(dsl::review_status.eq(review_status));
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

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let reviews: Vec<WorkspaceReview> = keyset!(
            scoped(),
            dsl::created_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select(WorkspaceReview::as_select())
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        let items = attach_assignees(self, reviews).await?;

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            DocumentReviewCursor {
                created_at: row.item.created_at.into(),
                id: row.item.id,
            }
        }))
    }

    async fn rename_review(
        &mut self,
        review_id: Uuid,
        display_name: String,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            // Renaming to the current name is a no-op: no write, no rename event.
            if review.display_name == display_name {
                return Ok(review);
            }

            let review = {
                use schema::workspace_reviews::{self, dsl};
                diesel::update(workspace_reviews::table.filter(dsl::id.eq(review_id)))
                    .set(dsl::display_name.eq(display_name.clone()))
                    .returning(WorkspaceReview::as_returning())
                    .get_result(conn)
                    .await
                    .map_err(Error::from)?
            };

            // The new name is the event's target so the timeline shows what it was
            // renamed to.
            let target = serde_json::json!({ "displayName": display_name });
            record_event(conn, &review, ReviewEventKind::Renamed, actor, Some(target)).await?;
            Ok(review)
        })
        .await
    }

    async fn delete_review(&mut self, review_id: Uuid) -> Result<()> {
        self.transaction(async |conn| {
            use schema::{workspace_review_comments, workspace_reviews};

            // Soft-delete the review and its live comments together, so a deleted
            // review leaves no live messages behind. (The FK cascade only fires on a
            // hard delete; comments are hidden here by their own `deleted_at`.)
            diesel::update(
                workspace_reviews::table
                    .filter(workspace_reviews::id.eq(review_id))
                    .filter(workspace_reviews::deleted_at.is_null()),
            )
            .set(workspace_reviews::deleted_at.eq(now))
            .execute(conn)
            .await
            .map_err(Error::from)?;

            diesel::update(
                workspace_review_comments::table
                    .filter(workspace_review_comments::review_id.eq(review_id))
                    .filter(workspace_review_comments::deleted_at.is_null()),
            )
            .set(workspace_review_comments::deleted_at.eq(now))
            .execute(conn)
            .await
            .map_err(Error::from)?;

            Ok(())
        })
        .await
    }

    async fn point_detection(
        &mut self,
        review_id: Uuid,
        detection_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            // Idempotent: already pointing at this detection records nothing.
            if review.detection_id == Some(detection_id) {
                return Ok(review);
            }

            let review = apply_update(
                conn,
                review_id,
                UpdateWorkspaceReview {
                    detection_id: Some(Some(detection_id)),
                    ..Default::default()
                },
            )
            .await?;

            record_event(
                conn,
                &review,
                ReviewEventKind::Redetected,
                actor,
                Some(serde_json::json!({ "detectionId": detection_id })),
            )
            .await?;
            Ok(review)
        })
        .await
    }

    async fn add_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
    ) -> Result<AssignmentOutcome> {
        self.transaction(async |conn| {
            // Lock the review row so the assignment insert serializes against a
            // concurrent assign/remove on the same review.
            let review = lock_review(conn, review_id).await?;

            let inserted = diesel::insert_into(schema::workspace_review_assignees::table)
                .values(&NewReviewAssignee {
                    review_id,
                    account_id,
                })
                .on_conflict_do_nothing()
                .execute(conn)
                .await
                .map_err(Error::from)?;

            // Already assigned: idempotent no-op, no event.
            if inserted == 0 {
                return Ok(AssignmentOutcome {
                    review,
                    changed: false,
                });
            }

            record_event(
                conn,
                &review,
                ReviewEventKind::Assigned,
                actor,
                Some(serde_json::json!({ "assigneeAccountId": account_id })),
            )
            .await?;

            // Assignment is metadata only: who is responsible, not what state the
            // review is in. The status is unchanged.
            Ok(AssignmentOutcome {
                review,
                changed: true,
            })
        })
        .await
    }

    async fn remove_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
        required_approvals: u32,
    ) -> Result<AssignmentOutcome> {
        self.transaction(async |conn| {
            use schema::workspace_review_assignees as wra;

            let review = lock_review(conn, review_id).await?;

            // Capture whether this assignee had approved before deleting the row: an
            // approved reviewer's removal drops one approval, so the review's status may
            // need to be recomputed below.
            let had_approved: Option<Option<jiff_diesel::Timestamp>> = wra::table
                .filter(wra::review_id.eq(review_id))
                .filter(wra::account_id.eq(account_id))
                .select(wra::approved_at)
                .first(conn)
                .await
                .optional()
                .map_err(Error::from)?;

            let deleted = diesel::delete(
                wra::table
                    .filter(wra::review_id.eq(review_id))
                    .filter(wra::account_id.eq(account_id)),
            )
            .execute(conn)
            .await
            .map_err(Error::from)?;

            // Not assigned: nothing to do.
            if deleted == 0 {
                return Ok(AssignmentOutcome {
                    review,
                    changed: false,
                });
            }

            record_event(
                conn,
                &review,
                ReviewEventKind::Unassigned,
                actor,
                Some(serde_json::json!({ "assigneeAccountId": account_id })),
            )
            .await?;

            // Removing an assignee who had not approved cannot change the decision, so
            // the status stands. Removing one who had approved drops an approval: if the
            // remaining count no longer meets quorum, demote the review under the same
            // lock so it never sits `approved` (or keeps an approved output) on fewer
            // approvals than the policy requires.
            let removed_an_approval = matches!(had_approved, Some(Some(_)));
            let mut review = review;
            if removed_an_approval
                && matches!(
                    review.review_status,
                    ReviewStatus::Approved | ReviewStatus::PartiallyApproved
                )
            {
                let approvals = count_approvals(conn, review_id).await?;
                if approvals < required_approvals.max(1) {
                    // Below quorum: an approved review returns to in_review and drops its
                    // approved output; a partially-approved review with approvals left
                    // stays partial, and one with none left returns to in_review.
                    let status = if approvals == 0 {
                        ReviewStatus::InReview
                    } else {
                        ReviewStatus::PartiallyApproved
                    };
                    let clear_output = review.review_status == ReviewStatus::Approved;
                    review = apply_update(
                        conn,
                        review_id,
                        UpdateWorkspaceReview {
                            review_status: Some(status),
                            redaction_id: clear_output.then_some(None),
                            ..Default::default()
                        },
                    )
                    .await?;
                }
            }

            Ok(AssignmentOutcome {
                review,
                changed: true,
            })
        })
        .await
    }

    async fn list_review_assignees(&mut self, review_id: Uuid) -> Result<Vec<AccountRefRow>> {
        use schema::{accounts, workspace_review_assignees as wra, workspace_review_assignees};

        workspace_review_assignees::table
            .inner_join(accounts::table.on(wra::account_id.eq(accounts::id)))
            .filter(wra::review_id.eq(review_id))
            .order(accounts::username.asc())
            .select((
                accounts::id,
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ))
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn submit_review(&mut self, review_id: Uuid, actor: Uuid) -> Result<StatusOutcome> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            // Re-check the status under the row lock: the domain pre-check runs on an
            // unlocked read, so only here is the repository authoritative. Only a draft
            // or pushed-back review submits; any other state (already in review, or a
            // concurrent approve won the race) is left untouched — in particular an
            // approved review must not revert to in_review with its approval stamps
            // still set.
            if !matches!(
                review.review_status,
                ReviewStatus::Draft | ReviewStatus::ChangesRequested
            ) {
                return Ok(StatusOutcome {
                    review,
                    changed: false,
                });
            }

            let review = update_status(conn, review_id, ReviewStatus::InReview).await?;
            record_event(conn, &review, ReviewEventKind::Submitted, actor, None).await?;
            Ok(StatusOutcome {
                review,
                changed: true,
            })
        })
        .await
    }

    async fn request_changes(&mut self, review_id: Uuid, actor: Uuid) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            // Re-check the status under the row lock: the domain pre-check runs on an
            // unlocked read, so only here, holding the lock, is the repository
            // authoritative. Changes can be requested only on a review awaiting a
            // decision; any other state (a concurrent approve won the race, or it was
            // already pushed back) is left untouched.
            if !matches!(
                review.review_status,
                ReviewStatus::InReview | ReviewStatus::PartiallyApproved
            ) {
                return Ok(review);
            }

            // A pushback invalidates the approvals accumulated against the prior
            // state, and the output they approved, so clear both before moving the
            // status (matching reopen, so an invalidated approval keeps no output).
            clear_approvals(conn, review_id).await?;
            let review = apply_update(
                conn,
                review_id,
                UpdateWorkspaceReview {
                    review_status: Some(ReviewStatus::ChangesRequested),
                    redaction_id: Some(None),
                    ..Default::default()
                },
            )
            .await?;
            record_event(
                conn,
                &review,
                ReviewEventKind::ChangesRequested,
                actor,
                None,
            )
            .await?;
            Ok(review)
        })
        .await
    }

    async fn approve_review(
        &mut self,
        review_id: Uuid,
        actor: Uuid,
        required_approvals: u32,
        output: Option<Uuid>,
    ) -> Result<ApprovalOutcome> {
        self.transaction(async |conn| {
            use schema::workspace_review_assignees as wra;

            let review = lock_review(conn, review_id).await?;

            // Re-check the status under the row lock: the domain pre-check runs on an
            // unlocked read, so a concurrent request-changes could have committed
            // between it and this lock. Only a review awaiting a decision can be
            // approved; otherwise return unchanged so the race's winner is not
            // overwritten.
            if !matches!(
                review.review_status,
                ReviewStatus::InReview | ReviewStatus::PartiallyApproved
            ) {
                let approvals = count_approvals(conn, review_id).await?;
                return Ok(ApprovalOutcome {
                    review,
                    recorded: false,
                    approvals,
                    rebind_rejected: false,
                });
            }

            // Once an earlier approval has bound an output, every later approval must
            // name that same redaction (or name none and inherit it). A quorum is only
            // meaningful if all its approvals endorse the same output; allowing a later
            // approver to rebind would mark the review approved for a redaction an
            // earlier approver never saw. Checked under the lock so the binding cannot
            // change between this read and the stamp below; the domain layer maps the
            // rejection to a 409.
            if let (Some(bound), Some(named)) = (review.redaction_id, output)
                && bound != named
            {
                let approvals = count_approvals(conn, review_id).await?;
                return Ok(ApprovalOutcome {
                    review,
                    recorded: false,
                    approvals,
                    rebind_rejected: true,
                });
            }

            // Stamp the caller's own assignee row, but only if they are assigned and
            // have not already approved (approved_at IS NULL). The row count tells us
            // whether this was a new approval; an un-assigned or already-approved
            // caller stamps nothing.
            let stamped = diesel::update(
                wra::table
                    .filter(wra::review_id.eq(review_id))
                    .filter(wra::account_id.eq(actor))
                    .filter(wra::approved_at.is_null()),
            )
            .set(wra::approved_at.eq(now))
            .execute(conn)
            .await
            .map_err(Error::from)?;

            // Count approved assignees now on the review (including any just stamped).
            let approvals = count_approvals(conn, review_id).await?;

            // Nothing stamped: the caller was not an assignee, or had already
            // approved. Idempotent, no event, no status move.
            if stamped == 0 {
                return Ok(ApprovalOutcome {
                    review,
                    recorded: false,
                    approvals,
                    rebind_rejected: false,
                });
            }

            // The approval event carries the output redaction the approver named (if
            // any), so the timeline shows which redaction was approved without a
            // separate event. Binding the output on each approval (partial or quorum)
            // means a later approver passing no output cannot drop an earlier binding.
            let approved_target =
                output.map(|redaction_id| serde_json::json!({ "redactionId": redaction_id }));
            record_event(
                conn,
                &review,
                ReviewEventKind::Approved,
                actor,
                approved_target,
            )
            .await?;

            // Quorum met → approved; otherwise it rests at partially_approved. The
            // output binding (if any) is applied either way; an absent output leaves
            // the current `redaction_id` untouched.
            let status = if approvals >= required_approvals.max(1) {
                ReviewStatus::Approved
            } else {
                ReviewStatus::PartiallyApproved
            };
            let review = apply_update(
                conn,
                review_id,
                UpdateWorkspaceReview {
                    review_status: Some(status),
                    redaction_id: output.map(Some),
                    ..Default::default()
                },
            )
            .await?;

            Ok(ApprovalOutcome {
                review,
                recorded: true,
                approvals,
                rebind_rejected: false,
            })
        })
        .await
    }

    async fn reopen_review(&mut self, review_id: Uuid, actor: Uuid) -> Result<StatusOutcome> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            // Re-check the status under the row lock: only an approved review reopens.
            // Any other state (a concurrent reopen already won) is left untouched and
            // reported unchanged, so the event does not fire twice.
            if review.review_status != ReviewStatus::Approved {
                return Ok(StatusOutcome {
                    review,
                    changed: false,
                });
            }

            // Reopening invalidates the approvals and the approved output: the review
            // returns to in_review so a new, better redaction can be approved.
            clear_approvals(conn, review_id).await?;
            let review = apply_update(
                conn,
                review_id,
                UpdateWorkspaceReview {
                    review_status: Some(ReviewStatus::InReview),
                    redaction_id: Some(None),
                    ..Default::default()
                },
            )
            .await?;
            record_event(conn, &review, ReviewEventKind::Reopened, actor, None).await?;
            Ok(StatusOutcome {
                review,
                changed: true,
            })
        })
        .await
    }

    async fn cursor_list_review_events(
        &mut self,
        review_id: Uuid,
        pagination: CursorPagination<ReviewEventCursor>,
    ) -> Result<CursorPage<WithActor<WorkspaceReviewEvent>>> {
        use schema::workspace_review_events::dsl;
        use schema::{accounts, workspace_review_events};

        let scoped = || {
            workspace_review_events::table
                .left_join(accounts::table.on(dsl::actor_account_id.eq(accounts::id.nullable())))
                .filter(dsl::review_id.eq(review_id))
                .into_boxed()
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

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let rows: Vec<(WorkspaceReviewEvent, Option<AccountRefRow>)> = keyset!(
            scoped(),
            dsl::created_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select((
            WorkspaceReviewEvent::as_select(),
            (
                accounts::id,
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            )
                .nullable(),
        ))
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        let items: Vec<WithActor<WorkspaceReviewEvent>> = rows
            .into_iter()
            .map(|(item, actor)| WithActor { item, actor })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            ReviewEventCursor {
                created_at: row.item.created_at.into(),
                id: row.item.id,
            }
        }))
    }
}

/// Loads a live review by id under a row lock (`FOR UPDATE`) or returns
/// `NotFound`, so a read-decide-write on its status serializes against concurrent
/// mutators (including a concurrent soft-delete). Call inside a transaction. Every
/// mutating method locks the review through this before acting, so a soft-deleted
/// review is uniformly `NotFound` to a mutation.
async fn lock_review(conn: &mut PgConnection, review_id: Uuid) -> Result<WorkspaceReview> {
    use schema::workspace_reviews::{self, dsl};

    workspace_reviews::table
        .filter(dsl::id.eq(review_id))
        .filter(dsl::deleted_at.is_null())
        .select(WorkspaceReview::as_select())
        .for_update()
        .first(conn)
        .await
        .map_err(Error::from)
}

/// Sets a review's status and returns the updated row.
async fn update_status(
    conn: &mut PgConnection,
    review_id: Uuid,
    status: ReviewStatus,
) -> Result<WorkspaceReview> {
    apply_update(
        conn,
        review_id,
        UpdateWorkspaceReview {
            review_status: Some(status),
            ..Default::default()
        },
    )
    .await
}

/// Applies a changeset to a review and returns the updated row. A `Default`
/// changeset (all `None`) touches only `updated_at` through the trigger.
async fn apply_update(
    conn: &mut PgConnection,
    review_id: Uuid,
    changes: UpdateWorkspaceReview,
) -> Result<WorkspaceReview> {
    use schema::workspace_reviews::{self, dsl};

    diesel::update(workspace_reviews::table.filter(dsl::id.eq(review_id)))
        .set(changes)
        .returning(WorkspaceReview::as_returning())
        .get_result(conn)
        .await
        .map_err(Error::from)
}

/// Counts the review's assignees that have approved (a stamped `approved_at`).
async fn count_approvals(conn: &mut PgConnection, review_id: Uuid) -> Result<u32> {
    use schema::workspace_review_assignees as wra;

    let count: i64 = wra::table
        .filter(wra::review_id.eq(review_id))
        .filter(wra::approved_at.is_not_null())
        .count()
        .get_result(conn)
        .await
        .map_err(Error::from)?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Clears every approval on a review (a pushback or reopen invalidates them),
/// leaving the assignments themselves in place.
async fn clear_approvals(conn: &mut PgConnection, review_id: Uuid) -> Result<()> {
    use schema::workspace_review_assignees as wra;

    diesel::update(wra::table.filter(wra::review_id.eq(review_id)))
        .set(wra::approved_at.eq(None::<jiff_diesel::Timestamp>))
        .execute(conn)
        .await
        .map_err(Error::from)?;
    Ok(())
}

/// Pairs each review with its assignees' account references in one grouped query
/// (no N+1): all assignees for the given reviews are loaded together and grouped
/// by review, preserving the input order.
async fn attach_assignees(
    conn: &mut PgConnection,
    reviews: Vec<WorkspaceReview>,
) -> Result<Vec<WithReviewers<WorkspaceReview>>> {
    use std::collections::HashMap;

    use schema::{accounts, workspace_review_assignees as wra, workspace_review_assignees};

    let review_ids: Vec<Uuid> = reviews.iter().map(|r| r.id).collect();
    let rows: Vec<(Uuid, AccountRefRow)> = workspace_review_assignees::table
        .inner_join(accounts::table.on(wra::account_id.eq(accounts::id)))
        .filter(wra::review_id.eq_any(&review_ids))
        .order(accounts::username.asc())
        .select((
            wra::review_id,
            (
                accounts::id,
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        ))
        .load(conn)
        .await
        .map_err(Error::from)?;

    let mut by_review: HashMap<Uuid, Vec<AccountRefRow>> = HashMap::new();
    for (review_id, account) in rows {
        by_review.entry(review_id).or_default().push(account);
    }

    Ok(reviews
        .into_iter()
        .map(|item| {
            let assignees = by_review.remove(&item.id).unwrap_or_default();
            WithReviewers { item, assignees }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::count_approvals;
    use crate::model::{
        NewAccount, NewWorkspaceRedaction, NewWorkspaceReview, NewWorkspaceReviewComment,
        UpdateWorkspaceReviewComment,
    };
    use crate::query::{
        AccountRepository, TimelineCursor, TimelineSource, WorkspaceRedactionRepository,
        WorkspaceReviewCommentRepository, WorkspaceReviewEventRepository,
        WorkspaceReviewRepository,
    };
    use crate::test_util::TestDatabase;
    use crate::types::{
        CursorPagination, Direction, DocumentReviewFilter, ReviewEventKind, ReviewStatus,
    };

    #[tokio::test]
    async fn create_lists_and_finds_reviews_per_document() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        // A document can have many concurrent reviews, one per audience.
        let public = conn
            .create_review(NewWorkspaceReview {
                workspace_id: seeded.workspace_id,
                document_id: seeded.document_id,
                detection_id: None,
                author_account_id: seeded.account_id,
                display_name: "Public release".to_owned(),
            })
            .await?;
        assert_eq!(public.review_status, ReviewStatus::Draft);
        assert_eq!(public.display_name, "Public release");

        let legal = conn
            .create_review(NewWorkspaceReview {
                workspace_id: seeded.workspace_id,
                document_id: seeded.document_id,
                detection_id: None,
                author_account_id: seeded.account_id,
                display_name: "Court filing".to_owned(),
            })
            .await?;
        assert_ne!(public.id, legal.id);

        // Both are listed for the document.
        let reviews = conn
            .list_document_reviews(seeded.workspace_id, seeded.document_id)
            .await?;
        assert_eq!(reviews.len(), 2);

        // Found by id within the workspace.
        let found = conn.find_review(seeded.workspace_id, legal.id).await?;
        assert_eq!(found.map(|r| r.id), Some(legal.id));

        // Opening a review records a `review.opened` event as the timeline's start.
        let kinds: Vec<_> = conn
            .list_review_events(legal.id)
            .await?
            .into_iter()
            .map(|(e, _)| e.kind)
            .collect();
        assert_eq!(kinds, vec![ReviewEventKind::Opened]);
        Ok(())
    }

    #[tokio::test]
    async fn multiple_concurrent_reviews_per_document_are_allowed() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        // The same document is reviewed for different audiences at the same time; both
        // open reviews coexist.
        let first = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        let second = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        assert_ne!(first.id, second.id);

        let reviews = conn
            .list_document_reviews(seeded.workspace_id, seeded.document_id)
            .await?;
        assert_eq!(reviews.len(), 2, "both concurrent reviews are listed");
        Ok(())
    }

    #[tokio::test]
    async fn rename_records_the_new_name() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;

        let renamed = conn
            .rename_review(review.id, "A title".to_owned(), seeded.account_id)
            .await?;
        assert_eq!(renamed.display_name, "A title");

        let kinds: Vec<_> = conn
            .list_review_events(review.id)
            .await?
            .into_iter()
            .map(|(e, _)| e.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![ReviewEventKind::Opened, ReviewEventKind::Renamed]
        );

        // Renaming to the current name is a no-op: no second rename event.
        conn.rename_review(review.id, "A title".to_owned(), seeded.account_id)
            .await?;
        let rename_events = conn
            .list_review_events(review.id)
            .await?
            .into_iter()
            .filter(|(e, _)| e.kind == ReviewEventKind::Renamed)
            .count();
        assert_eq!(rename_events, 1, "an identical rename records no new event");
        Ok(())
    }

    #[tokio::test]
    async fn comment_edit_and_delete() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;

        let comment = conn
            .create_comment(NewWorkspaceReviewComment::test(
                review.id,
                seeded.account_id,
            ))
            .await?;
        assert_eq!(comment.review_id, review.id);

        let edited = conn
            .update_comment_body(
                comment.id,
                UpdateWorkspaceReviewComment {
                    body: Some("Edited.".to_owned()),
                },
            )
            .await?;
        assert_eq!(edited.body, "Edited.");

        conn.delete_comment(comment.id).await?;
        assert!(
            conn.find_comment_in_workspace(seeded.workspace_id, comment.id)
                .await?
                .is_none()
        );
        // The review still exists after deleting a message.
        assert!(
            conn.find_review(seeded.workspace_id, review.id)
                .await?
                .is_some()
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_review_hides_it_and_its_comments() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.create_comment(NewWorkspaceReviewComment::test(
            review.id,
            seeded.account_id,
        ))
        .await?;

        conn.delete_review(review.id).await?;
        assert!(
            conn.find_review(seeded.workspace_id, review.id)
                .await?
                .is_none()
        );
        assert_eq!(conn.list_review_comments(review.id).await?, vec![]);
        Ok(())
    }

    #[tokio::test]
    async fn create_reply_is_unique_per_triggering_comment() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        let trigger = conn
            .create_comment(NewWorkspaceReviewComment {
                parent_id: None,
                review_id: review.id,
                author_account_id: seeded.account_id,
                body: "@assistant help".to_owned(),
            })
            .await?;

        let reply = |body: &str| NewWorkspaceReviewComment {
            review_id: review.id,
            author_account_id: seeded.account_id,
            parent_id: Some(trigger.id),
            body: body.to_owned(),
        };

        // The first reply to the triggering comment posts.
        let first = conn.create_reply(reply("first")).await?;
        assert!(first.is_some());

        // A second reply to the same comment is rejected by the partial unique index
        // and reported as "already replied" (None), never a duplicate.
        let second = conn.create_reply(reply("second")).await?;
        assert!(second.is_none());

        // After the first reply is soft-deleted, a new reply may be posted again.
        conn.delete_comment(first.unwrap().id).await?;
        let third = conn.create_reply(reply("third")).await?;
        assert!(third.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn lock_comment_returns_only_live_in_scope_comments() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        let comment = conn
            .create_comment(NewWorkspaceReviewComment {
                parent_id: None,
                review_id: review.id,
                author_account_id: seeded.account_id,
                body: "@assistant help".to_owned(),
            })
            .await?;

        // A live comment in the workspace is locked and returned.
        let locked = conn
            .lock_comment_in_workspace(seeded.workspace_id, comment.id)
            .await?;
        assert_eq!(locked.map(|c| c.id), Some(comment.id));

        // A different workspace does not see it (scoped through its review).
        assert!(
            conn.lock_comment_in_workspace(uuid::Uuid::new_v4(), comment.id)
                .await?
                .is_none(),
            "a comment outside the workspace is not locked"
        );

        // Once soft-deleted it is gone: this is what keeps a reply from landing
        // under a parent deleted while the model ran.
        conn.delete_comment(comment.id).await?;
        assert!(
            conn.lock_comment_in_workspace(seeded.workspace_id, comment.id)
                .await?
                .is_none(),
            "a soft-deleted comment is not locked"
        );
        Ok(())
    }

    #[tokio::test]
    async fn assignment_is_metadata_and_records_timeline_without_changing_status()
    -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let bob = {
            let mut conn = db.client.get_connection().await?;
            conn.create_account(NewAccount::test()).await?.id
        };
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;

        // Assigning the first reviewer records the assignment but leaves the review
        // in `draft`: assignment is metadata, not a lifecycle trigger.
        let a = conn
            .add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        assert!(a.changed);
        assert_eq!(a.review.review_status, ReviewStatus::Draft);

        // A second assignee; re-adding the same one is an idempotent no-op.
        conn.add_assignee(review.id, bob, seeded.account_id).await?;
        let again = conn.add_assignee(review.id, bob, seeded.account_id).await?;
        assert!(
            !again.changed,
            "re-adding an existing assignee changes nothing"
        );
        assert_eq!(again.review.review_status, ReviewStatus::Draft);
        assert_eq!(conn.list_review_assignees(review.id).await?.len(), 2);

        // Removing assignees who have not approved never moves the status.
        let one_left = conn
            .remove_assignee(review.id, bob, seeded.account_id, 1)
            .await?;
        assert!(one_left.changed);
        assert_eq!(one_left.review.review_status, ReviewStatus::Draft);
        let none_left = conn
            .remove_assignee(review.id, seeded.account_id, seeded.account_id, 1)
            .await?;
        assert_eq!(none_left.review.review_status, ReviewStatus::Draft);
        assert_eq!(conn.list_review_assignees(review.id).await?, vec![]);

        // Removing a not-assigned reviewer is a no-op (changed = false).
        let noop = conn
            .remove_assignee(review.id, bob, seeded.account_id, 1)
            .await?;
        assert!(!noop.changed, "removing a non-assignee changes nothing");

        // The timeline records opened, then two assigns and two unassigns.
        let kinds: Vec<_> = conn
            .cursor_list_review_events(
                review.id,
                CursorPagination::new(50).with_direction(Direction::Ascending),
            )
            .await?
            .items
            .into_iter()
            .map(|e| e.item.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                ReviewEventKind::Opened,
                ReviewEventKind::Assigned,
                ReviewEventKind::Assigned,
                ReviewEventKind::Unassigned,
                ReviewEventKind::Unassigned,
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn lifecycle_moves_on_decisions_not_assignment() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let fixture = db.seed_detection_and_redaction().await;
        let seeded = fixture.seeded;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        assert_eq!(review.review_status, ReviewStatus::Draft);

        // submit: draft -> in_review.
        let submitted = conn.submit_review(review.id, seeded.account_id).await?;
        assert!(submitted.changed);
        assert_eq!(submitted.review.review_status, ReviewStatus::InReview);

        // request-changes: in_review -> changes_requested.
        let pushed_back = conn.request_changes(review.id, seeded.account_id).await?;
        assert_eq!(pushed_back.review_status, ReviewStatus::ChangesRequested);

        // resubmit (submit from changes_requested): -> in_review.
        let resubmitted = conn.submit_review(review.id, seeded.account_id).await?;
        assert!(resubmitted.changed);
        assert_eq!(resubmitted.review.review_status, ReviewStatus::InReview);

        // approve with a single required approval: -> approved, output recorded. A
        // reviewer approves by stamping their own assignee row, so assign first.
        conn.add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        let redaction_id = fixture.redaction_id;
        let outcome = conn
            .approve_review(review.id, seeded.account_id, 1, Some(redaction_id))
            .await?;
        assert!(outcome.recorded);
        assert_eq!(outcome.approvals, 1);
        assert_eq!(outcome.review.review_status, ReviewStatus::Approved);
        assert_eq!(outcome.review.redaction_id, Some(redaction_id));

        // reopen: approved -> in_review, approvals + output cleared.
        let reopened = conn.reopen_review(review.id, seeded.account_id).await?;
        assert!(reopened.changed);
        assert_eq!(reopened.review.review_status, ReviewStatus::InReview);
        assert_eq!(reopened.review.redaction_id, None);

        let kinds: Vec<_> = conn
            .cursor_list_review_events(
                review.id,
                CursorPagination::new(50).with_direction(Direction::Ascending),
            )
            .await?
            .items
            .into_iter()
            .map(|e| e.item.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                ReviewEventKind::Opened,
                ReviewEventKind::Submitted,
                ReviewEventKind::ChangesRequested,
                ReviewEventKind::Submitted,
                ReviewEventKind::Assigned,
                ReviewEventKind::Approved,
                ReviewEventKind::Reopened,
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn two_approvals_pass_through_partially_approved() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let fixture = db.seed_detection_and_redaction().await;
        let seeded = fixture.seeded;
        let redaction_id = fixture.redaction_id;
        let bob = {
            let mut conn = db.client.get_connection().await?;
            conn.create_account(NewAccount::test()).await?.id
        };
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.submit_review(review.id, seeded.account_id).await?;

        // Both reviewers must be assigned to approve (approval stamps the assignee
        // row).
        conn.add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        conn.add_assignee(review.id, bob, seeded.account_id).await?;

        // First of two required approvals: partially_approved, and the named output
        // is bound immediately (so a later approver passing none cannot drop it).
        let first = conn
            .approve_review(review.id, seeded.account_id, 2, Some(redaction_id))
            .await?;
        assert!(first.recorded);
        assert_eq!(first.approvals, 1);
        assert_eq!(first.review.review_status, ReviewStatus::PartiallyApproved);
        assert_eq!(first.review.redaction_id, Some(redaction_id));

        // The same reviewer approving again is an idempotent no-op.
        let again = conn
            .approve_review(review.id, seeded.account_id, 2, Some(redaction_id))
            .await?;
        assert!(!again.recorded);
        assert_eq!(again.approvals, 1);
        assert_eq!(again.review.review_status, ReviewStatus::PartiallyApproved);

        // Requesting changes while partially approved clears the accumulated approval
        // (and its bound output), so quorum must be met afresh.
        let pushed_back = conn.request_changes(review.id, seeded.account_id).await?;
        assert_eq!(pushed_back.review_status, ReviewStatus::ChangesRequested);
        assert_eq!(
            pushed_back.redaction_id, None,
            "the invalidated approval's output is cleared"
        );
        conn.submit_review(review.id, seeded.account_id).await?;
        let after = conn
            .approve_review(review.id, seeded.account_id, 2, None)
            .await?;
        assert_eq!(
            after.approvals, 1,
            "approvals were reset by request-changes"
        );
        assert_eq!(after.review.review_status, ReviewStatus::PartiallyApproved);

        // The second distinct approval now meets quorum again: approved.
        let second = conn.approve_review(review.id, bob, 2, None).await?;
        assert!(second.recorded);
        assert_eq!(second.approvals, 2);
        assert_eq!(second.review.review_status, ReviewStatus::Approved);
        Ok(())
    }

    #[tokio::test]
    async fn a_quorum_approval_without_output_keeps_an_earlier_one() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let fixture = db.seed_detection_and_redaction().await;
        let seeded = fixture.seeded;
        let redaction_id = fixture.redaction_id;
        let bob = {
            let mut conn = db.client.get_connection().await?;
            conn.create_account(NewAccount::test()).await?.id
        };
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.submit_review(review.id, seeded.account_id).await?;
        conn.add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        conn.add_assignee(review.id, bob, seeded.account_id).await?;

        // The first approver names the output; the quorum-meeting approver names none.
        conn.approve_review(review.id, seeded.account_id, 2, Some(redaction_id))
            .await?;
        let approved = conn.approve_review(review.id, bob, 2, None).await?;

        // The review is approved and keeps the output the first approver bound; the
        // absent output on the final approval does not clear it.
        assert_eq!(approved.review.review_status, ReviewStatus::Approved);
        assert_eq!(approved.review.redaction_id, Some(redaction_id));
        Ok(())
    }

    #[tokio::test]
    async fn a_later_approval_cannot_rebind_the_output() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let fixture = db.seed_detection_and_redaction().await;
        let seeded = fixture.seeded;
        let redaction_id = fixture.redaction_id;
        let bob = {
            let mut conn = db.client.get_connection().await?;
            conn.create_account(NewAccount::test()).await?.id
        };
        let mut conn = db.client.get_connection().await?;

        // A second redaction of the same detection: a valid, different output.
        let other_redaction = conn
            .create_redaction(NewWorkspaceRedaction::test(
                fixture.detection_id,
                seeded.account_id,
            ))
            .await?
            .id;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.submit_review(review.id, seeded.account_id).await?;
        conn.add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        conn.add_assignee(review.id, bob, seeded.account_id).await?;

        // The first approver binds one output.
        conn.approve_review(review.id, seeded.account_id, 2, Some(redaction_id))
            .await?;

        // A second approver naming a different output is rejected under the lock:
        // nothing is stamped, the binding stands, and the status does not advance.
        let rebind = conn
            .approve_review(review.id, bob, 2, Some(other_redaction))
            .await?;
        assert!(rebind.rebind_rejected);
        assert!(!rebind.recorded);
        assert_eq!(rebind.approvals, 1);
        assert_eq!(rebind.review.review_status, ReviewStatus::PartiallyApproved);
        assert_eq!(rebind.review.redaction_id, Some(redaction_id));

        // The second approver endorsing the already-bound output meets quorum.
        let agreed = conn
            .approve_review(review.id, bob, 2, Some(redaction_id))
            .await?;
        assert!(agreed.recorded);
        assert_eq!(agreed.review.review_status, ReviewStatus::Approved);
        assert_eq!(agreed.review.redaction_id, Some(redaction_id));
        Ok(())
    }

    #[tokio::test]
    async fn submit_is_a_no_op_once_a_review_is_approved() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let fixture = db.seed_detection_and_redaction().await;
        let seeded = fixture.seeded;
        let redaction_id = fixture.redaction_id;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.submit_review(review.id, seeded.account_id).await?;
        conn.add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        conn.approve_review(review.id, seeded.account_id, 1, Some(redaction_id))
            .await?;

        // A submit racing in after the review is approved finds it in a non-submittable
        // state under the lock: it changes nothing and leaves the approval intact.
        let outcome = conn.submit_review(review.id, seeded.account_id).await?;
        assert!(!outcome.changed, "an approved review does not re-submit");
        assert_eq!(outcome.review.review_status, ReviewStatus::Approved);
        assert_eq!(outcome.review.redaction_id, Some(redaction_id));
        assert_eq!(
            count_approvals(&mut conn, review.id).await?,
            1,
            "the approval stamp survives the no-op submit"
        );
        Ok(())
    }

    #[tokio::test]
    async fn removing_an_approved_assignee_demotes_below_quorum() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let fixture = db.seed_detection_and_redaction().await;
        let seeded = fixture.seeded;
        let redaction_id = fixture.redaction_id;
        let bob = {
            let mut conn = db.client.get_connection().await?;
            conn.create_account(NewAccount::test()).await?.id
        };
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.submit_review(review.id, seeded.account_id).await?;
        conn.add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        conn.add_assignee(review.id, bob, seeded.account_id).await?;

        // Two approvals meet a quorum of two: approved, output bound.
        conn.approve_review(review.id, seeded.account_id, 2, Some(redaction_id))
            .await?;
        let approved = conn.approve_review(review.id, bob, 2, None).await?;
        assert_eq!(approved.review.review_status, ReviewStatus::Approved);

        // Removing one approver drops to one approval, below the quorum of two: the
        // review returns to partially_approved and its approved output is cleared.
        let outcome = conn
            .remove_assignee(review.id, bob, seeded.account_id, 2)
            .await?;
        assert!(outcome.changed);
        assert_eq!(
            outcome.review.review_status,
            ReviewStatus::PartiallyApproved
        );
        assert_eq!(
            outcome.review.redaction_id, None,
            "the approved output is cleared when the review falls below quorum"
        );

        // Removing the last approver too drops to zero approvals: back to in_review.
        let outcome = conn
            .remove_assignee(review.id, seeded.account_id, seeded.account_id, 2)
            .await?;
        assert_eq!(outcome.review.review_status, ReviewStatus::InReview);
        Ok(())
    }

    #[tokio::test]
    async fn queue_filters_by_assignee_and_status() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        let a = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.add_assignee(a.id, seeded.account_id, seeded.account_id)
            .await?;
        conn.submit_review(a.id, seeded.account_id).await?;

        // Filter to the reviewer's in_review queue: only `a` (assigned and submitted).
        let queue = conn
            .cursor_list_reviews(
                seeded.workspace_id,
                CursorPagination::new(50),
                &DocumentReviewFilter {
                    assignee_account_id: Some(seeded.account_id),
                    review_status: Some(ReviewStatus::InReview),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(queue.items.len(), 1);
        assert_eq!(queue.items[0].item.id, a.id);
        assert_eq!(queue.items[0].assignees.len(), 1);
        Ok(())
    }

    /// A merged-timeline page: one entry with its sort key, mirroring how the
    /// handler interleaves the two streams. `(created_at, source, id)`.
    type Entry = (jiff::Timestamp, TimelineSource, uuid::Uuid);

    /// Fetches one page of the merged timeline (comments + events) after `cursor`,
    /// mirroring the handler: pull `limit + 1` from each stream, merge by
    /// `(created_at, source, id)`, keep `limit`, and return the next cursor.
    async fn timeline_page(
        conn: &mut crate::PgConn,
        review_id: uuid::Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> anyhow::Result<(Vec<Entry>, Option<TimelineCursor>)> {
        let fetch = limit + 1;
        let comments = conn
            .list_review_comments_after(review_id, after, fetch)
            .await?;
        let events = conn
            .list_review_events_after(review_id, after, fetch)
            .await?;

        let mut merged: Vec<Entry> = Vec::new();
        merged.extend(
            comments
                .iter()
                .map(|c| (c.item.created_at.into(), TimelineSource::Comment, c.item.id)),
        );
        merged.extend(
            events
                .iter()
                .map(|(e, _)| (e.created_at.into(), TimelineSource::Event, e.id)),
        );
        merged.sort();

        let next = if i64::try_from(merged.len()).unwrap_or(i64::MAX) > limit {
            merged.truncate(usize::try_from(limit).unwrap_or(0));
            merged
                .last()
                .map(|&(created_at, source, id)| TimelineCursor {
                    created_at,
                    source,
                    id,
                })
        } else {
            None
        };
        Ok((merged, next))
    }

    #[tokio::test]
    async fn timeline_pages_comments_and_events_in_one_order() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        // A review with a known set of timeline entries: opening (1 event), two
        // comments, and a rename (1 event) = 4 entries.
        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.create_comment(NewWorkspaceReviewComment::test(
            review.id,
            seeded.account_id,
        ))
        .await?;
        conn.create_comment(NewWorkspaceReviewComment::test(
            review.id,
            seeded.account_id,
        ))
        .await?;
        conn.rename_review(review.id, "Renamed".to_owned(), seeded.account_id)
            .await?;

        // The full merged timeline (a big first page) is every entry in order.
        let (all, _) = timeline_page(&mut conn, review.id, None, 50).await?;
        assert_eq!(all.len(), 4);
        // It is sorted ascending by (created_at, source, id).
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(all, sorted);

        // Paging in windows of 2 walks the same order with no gaps or repeats.
        let mut paged: Vec<Entry> = Vec::new();
        let mut cursor: Option<TimelineCursor> = None;
        loop {
            let (page, next) = timeline_page(&mut conn, review.id, cursor.as_ref(), 2).await?;
            paged.extend(page);
            match next {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        assert_eq!(paged, all);
        Ok(())
    }
}
