//! Workspace review repository. A review is a named discussion on a document with
//! a manual sign-off lifecycle (0..N per document): an author, a title, a stream of
//! comments and timeline events, and a status (`needs_review` → `in_review` →
//! `resolved`, reopen). It references — does not own — the detections and
//! redactions done for it (via the link tables). A review is created explicitly
//! (not auto-created by detection/redaction). Opening a review records the
//! `review.opened` event; renaming, linking, assignment, verification, and reopen
//! each record their own event on the review's one timeline
//! ([`WorkspaceReviewEvent`]), which the reader interleaves with the comments.
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
    NewReviewAssignee, NewReviewDetection, NewReviewRedaction, NewWorkspaceReview, WorkspaceReview,
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
    /// Opens a new review on a document at [`ReviewStatus::NeedsReview`], recording
    /// the `review.opened` timeline event, in one transaction, and returns the
    /// review. `display_name` is the review's title. A document may have any number
    /// of reviews.
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

    /// References a detection from a review (idempotent), recording a
    /// `detection.linked` event on first link. Returns the review.
    fn link_detection(
        &mut self,
        review_id: Uuid,
        detection_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// References a redaction from a review (idempotent), recording a
    /// `redaction.linked` event on first link. Returns the review.
    fn link_redaction(
        &mut self,
        review_id: Uuid,
        redaction_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Assigns a reviewer to a review (idempotent). Records an `assigned` event on
    /// first assignment and, when this is the first assignee of a non-resolved
    /// review, moves it to `in_review`. The [`AssignmentOutcome`] reports the review
    /// and whether anything changed (`false` when the reviewer was already assigned).
    fn add_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<AssignmentOutcome>> + Send;

    /// Removes a reviewer from a review. Records an `unassigned` event when a link
    /// existed and, when this clears the last assignee of a non-resolved review,
    /// returns it to `needs_review`. The [`AssignmentOutcome`] reports the review and
    /// whether a link was actually removed (`false` when the reviewer was not
    /// assigned).
    fn remove_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<AssignmentOutcome>> + Send;

    /// Lists a review's assignees' account references.
    fn list_review_assignees(
        &mut self,
        review_id: Uuid,
    ) -> impl Future<Output = Result<Vec<AccountRefRow>>> + Send;

    /// Verifies a review, moving it to [`Resolved`](ReviewStatus::Resolved) and
    /// recording a `verified` event. Locks the review first; an already-resolved
    /// review is returned unchanged (the domain layer pre-checks to report a clean
    /// conflict, so this only guards a concurrent double-verify).
    fn verify_review(
        &mut self,
        review_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Reopens a resolved review back to [`NeedsReview`](ReviewStatus::NeedsReview),
    /// recording a `reopened` event. A no-op returning the review when it is not
    /// resolved.
    fn reopen_review(
        &mut self,
        review_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

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

/// The result of an assignee add/remove: the (possibly status-updated) review and
/// whether the operation actually changed anything (`false` for an idempotent
/// no-op — an already-assigned add or a not-assigned remove).
#[derive(Debug, Clone)]
pub struct AssignmentOutcome {
    /// The review after the operation.
    pub review: WorkspaceReview,
    /// Whether a link was added or removed (vs an idempotent no-op).
    pub changed: bool,
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
            // begins with an explicit `review.opened` entry.
            record_event(
                conn,
                &review,
                ReviewEventKind::Opened,
                review.author_account_id,
                None,
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

    async fn link_detection(
        &mut self,
        review_id: Uuid,
        detection_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            let inserted = diesel::insert_into(schema::workspace_review_detections::table)
                .values(&NewReviewDetection {
                    review_id,
                    detection_id,
                })
                .on_conflict_do_nothing()
                .execute(conn)
                .await
                .map_err(Error::from)?;

            // Record the link only when it is new (idempotent re-link is silent).
            if inserted > 0 {
                record_event(
                    conn,
                    &review,
                    ReviewEventKind::DetectionLinked,
                    actor,
                    Some(serde_json::json!({ "detectionId": detection_id })),
                )
                .await?;
            }
            Ok(review)
        })
        .await
    }

    async fn link_redaction(
        &mut self,
        review_id: Uuid,
        redaction_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            let inserted = diesel::insert_into(schema::workspace_review_redactions::table)
                .values(&NewReviewRedaction {
                    review_id,
                    redaction_id,
                })
                .on_conflict_do_nothing()
                .execute(conn)
                .await
                .map_err(Error::from)?;

            if inserted > 0 {
                record_event(
                    conn,
                    &review,
                    ReviewEventKind::RedactionLinked,
                    actor,
                    Some(serde_json::json!({ "redactionId": redaction_id })),
                )
                .await?;
            }
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
            // Lock the review row so the assignment insert and the status decision
            // that reads from it serialize against a concurrent assign/remove/verify.
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

            // Already assigned: idempotent no-op, no event, no status change.
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

            // The first assignee of a not-yet-resolved review takes it to
            // `in_review`; a resolved review keeps its status (reopen is explicit).
            let review = if review.review_status == ReviewStatus::NeedsReview {
                update_status(conn, review_id, ReviewStatus::InReview).await?
            } else {
                review
            };
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
    ) -> Result<AssignmentOutcome> {
        self.transaction(async |conn| {
            use schema::workspace_review_assignees as wra;

            let review = lock_review(conn, review_id).await?;

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

            // Clearing the last assignee of an in-review review returns it to
            // `needs_review`; a resolved review keeps its status.
            let review = if review.review_status == ReviewStatus::InReview {
                let remaining: i64 = wra::table
                    .filter(wra::review_id.eq(review_id))
                    .count()
                    .get_result(conn)
                    .await
                    .map_err(Error::from)?;
                if remaining == 0 {
                    update_status(conn, review_id, ReviewStatus::NeedsReview).await?
                } else {
                    review
                }
            } else {
                review
            };
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

    async fn verify_review(&mut self, review_id: Uuid, actor: Uuid) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            // Already resolved: idempotent no-op (the domain layer pre-checks and
            // reports a 409, so this only guards a concurrent double-verify).
            if review.review_status == ReviewStatus::Resolved {
                return Ok(review);
            }

            let review = update_status(conn, review_id, ReviewStatus::Resolved).await?;
            record_event(conn, &review, ReviewEventKind::Verified, actor, None).await?;
            Ok(review)
        })
        .await
    }

    async fn reopen_review(&mut self, review_id: Uuid, actor: Uuid) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = lock_review(conn, review_id).await?;

            // Only a resolved review reopens; otherwise leave it as is.
            if review.review_status != ReviewStatus::Resolved {
                return Ok(review);
            }

            let review = update_status(conn, review_id, ReviewStatus::NeedsReview).await?;
            record_event(conn, &review, ReviewEventKind::Reopened, actor, None).await?;
            Ok(review)
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
    use schema::workspace_reviews::{self, dsl};

    diesel::update(workspace_reviews::table.filter(dsl::id.eq(review_id)))
        .set(dsl::review_status.eq(status))
        .returning(WorkspaceReview::as_returning())
        .get_result(conn)
        .await
        .map_err(Error::from)
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
    use crate::model::{
        NewAccount, NewWorkspaceReview, NewWorkspaceReviewComment, UpdateWorkspaceReviewComment,
    };
    use crate::query::{
        AccountRepository, TimelineCursor, TimelineSource, WorkspaceReviewCommentRepository,
        WorkspaceReviewEventRepository, WorkspaceReviewRepository,
    };
    use crate::test_util::TestDatabase;
    use crate::types::{
        CursorPagination, Direction, DocumentReviewFilter, ReviewEventKind, ReviewStatus,
    };

    #[tokio::test]
    async fn create_lists_and_finds_reviews_per_document() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        // A document can have many reviews (0..N).
        let public = conn
            .create_review(NewWorkspaceReview {
                workspace_id: seeded.workspace_id,
                document_id: seeded.document_id,
                author_account_id: seeded.account_id,
                display_name: "Public release".to_owned(),
            })
            .await?;
        let legal = conn
            .create_review(NewWorkspaceReview {
                workspace_id: seeded.workspace_id,
                document_id: seeded.document_id,
                author_account_id: seeded.account_id,
                display_name: "Court filing".to_owned(),
            })
            .await?;
        assert_ne!(public.id, legal.id);
        assert_eq!(public.review_status, ReviewStatus::NeedsReview);
        assert_eq!(public.display_name, "Public release");

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
            .list_review_events(public.id)
            .await?
            .into_iter()
            .map(|(e, _)| e.kind)
            .collect();
        assert_eq!(kinds, vec![ReviewEventKind::Opened]);
        Ok(())
    }

    #[tokio::test]
    async fn rename_records_the_new_name() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
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
        let seeded = db.seed_pipeline_and_document().await;
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
        let seeded = db.seed_pipeline_and_document().await;
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
        assert!(conn.list_review_comments(review.id).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn create_reply_is_unique_per_triggering_comment() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
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
    async fn assignees_drive_status_and_the_timeline() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
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

        // First assignee moves needs_review -> in_review (a real change).
        let a = conn
            .add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        assert!(a.changed);
        assert_eq!(a.review.review_status, ReviewStatus::InReview);

        // A second assignee is added (still in_review); re-adding the same one is a
        // no-op (idempotent), reported as `changed = false`.
        conn.add_assignee(review.id, bob, seeded.account_id).await?;
        let again = conn.add_assignee(review.id, bob, seeded.account_id).await?;
        assert!(
            !again.changed,
            "re-adding an existing assignee changes nothing"
        );
        assert_eq!(again.review.review_status, ReviewStatus::InReview);
        let assignees = conn.list_review_assignees(review.id).await?;
        assert_eq!(assignees.len(), 2, "two distinct assignees");

        // Removing one of two leaves it in_review.
        let one_left = conn
            .remove_assignee(review.id, bob, seeded.account_id)
            .await?;
        assert!(one_left.changed);
        assert_eq!(one_left.review.review_status, ReviewStatus::InReview);

        // Removing the last returns it to needs_review.
        let none_left = conn
            .remove_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        assert_eq!(none_left.review.review_status, ReviewStatus::NeedsReview);
        assert!(conn.list_review_assignees(review.id).await?.is_empty());

        // Removing a not-assigned reviewer is a no-op (changed = false).
        let noop = conn
            .remove_assignee(review.id, bob, seeded.account_id)
            .await?;
        assert!(!noop.changed, "removing a non-assignee changes nothing");

        // The timeline records opened, then the assign/unassign activity (oldest
        // first): two assigns (the idempotent re-add records nothing), two unassigns.
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
    async fn assigning_a_resolved_review_keeps_it_resolved() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        let resolved = conn.verify_review(review.id, seeded.account_id).await?;
        assert_eq!(resolved.review_status, ReviewStatus::Resolved);

        // Assigning a reviewer to a resolved review records the assignment but does
        // NOT silently un-resolve verified work.
        let assigned = conn
            .add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        assert_eq!(assigned.review.review_status, ReviewStatus::Resolved);
        assert_eq!(conn.list_review_assignees(review.id).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn queue_filters_by_assignee_and_status() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let a = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        let _b = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        conn.add_assignee(a.id, seeded.account_id, seeded.account_id)
            .await?;

        // Filter to the reviewer's in_review queue: only `a`.
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
        let seeded = db.seed_pipeline_and_document().await;
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
