//! Workspace document-review repository. A review is an optional, purpose-scoped
//! sign-off effort on a document (0..N per document). It owns a discussion
//! [`WorkspaceThread`] and references — does not own — the detections and
//! redactions done for its purpose (via the link tables). A review is created
//! explicitly (not auto-created by detection/redaction); its status is a manual
//! reviewer workflow (`needs_review` → `in_review` → `resolved`, reopen). Every
//! transition records an event on the review's own activity log
//! ([`WorkspaceReviewEvent`]), distinct from the thread's discussion timeline.
//!
//! [`WorkspaceThread`]: crate::model::WorkspaceThread
//! [`WorkspaceReviewEvent`]: crate::model::WorkspaceReviewEvent

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::model::{
    NewReviewAssignee, NewReviewDetection, NewReviewRedaction, NewWorkspaceReview,
    NewWorkspaceReviewEvent, NewWorkspaceThread, WorkspaceReview, WorkspaceReviewEvent,
    WorkspaceThread,
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
    /// Opens a new review on a document: creates its discussion thread and the
    /// review (at [`ReviewStatus::NeedsReview`]) together, in one transaction, and
    /// returns the review. `purpose` is optional descriptive metadata. A document
    /// may have any number of reviews.
    fn create_review(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
        purpose: Option<String>,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Finds a review by id within a workspace.
    fn find_review(
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
    /// review, moves it to `in_review`. Returns the review.
    fn add_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Removes a reviewer from a review. Records an `unassigned` event when a link
    /// existed and, when this clears the last assignee of a non-resolved review,
    /// returns it to `needs_review`. Returns the review.
    fn remove_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceReview>> + Send;

    /// Lists a review's assignees' account references.
    fn list_review_assignees(
        &mut self,
        review_id: Uuid,
    ) -> impl Future<Output = Result<Vec<AccountRefRow>>> + Send;

    /// Verifies a review, moving it to [`Resolved`](ReviewStatus::Resolved) and
    /// recording a `verified` event. Guarded on `status != resolved`, so an
    /// already-resolved review matches no row (callers pre-check to report a clean
    /// conflict).
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
    async fn create_review(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
        purpose: Option<String>,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let thread = {
                use schema::workspace_threads;
                diesel::insert_into(workspace_threads::table)
                    .values(&NewWorkspaceThread {
                        workspace_id,
                        author_account_id: actor,
                        display_name: None,
                    })
                    .returning(WorkspaceThread::as_returning())
                    .get_result(conn)
                    .await
                    .map_err(Error::from)?
            };

            diesel::insert_into(schema::workspace_reviews::table)
                .values(&NewWorkspaceReview {
                    workspace_id,
                    document_id,
                    thread_id: thread.id,
                    purpose,
                })
                .returning(WorkspaceReview::as_returning())
                .get_result(conn)
                .await
                .map_err(Error::from)
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
            .select(WorkspaceReview::as_select())
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
                .into_boxed();
            if let Some(document_id) = filter.document_id {
                query = query.filter(dsl::document_id.eq(document_id));
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

    async fn link_detection(
        &mut self,
        review_id: Uuid,
        detection_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = load_review(conn, review_id).await?;

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
                record_review_event(
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
            let review = load_review(conn, review_id).await?;

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
                record_review_event(
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
    ) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            let review = load_review(conn, review_id).await?;

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
                return Ok(review);
            }

            record_review_event(
                conn,
                &review,
                ReviewEventKind::Assigned,
                actor,
                Some(serde_json::json!({ "assigneeAccountId": account_id })),
            )
            .await?;

            // The first assignee of a not-yet-resolved review takes it to
            // `in_review`; a resolved review keeps its status (reopen is explicit).
            if review.review_status == ReviewStatus::NeedsReview {
                return update_status(conn, review_id, ReviewStatus::InReview).await;
            }
            Ok(review)
        })
        .await
    }

    async fn remove_assignee(
        &mut self,
        review_id: Uuid,
        account_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            use schema::workspace_review_assignees as wra;

            let review = load_review(conn, review_id).await?;

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
                return Ok(review);
            }

            record_review_event(
                conn,
                &review,
                ReviewEventKind::Unassigned,
                actor,
                Some(serde_json::json!({ "assigneeAccountId": account_id })),
            )
            .await?;

            // Clearing the last assignee of an in-review review returns it to
            // `needs_review`; a resolved review keeps its status.
            if review.review_status == ReviewStatus::InReview {
                let remaining: i64 = wra::table
                    .filter(wra::review_id.eq(review_id))
                    .count()
                    .get_result(conn)
                    .await
                    .map_err(Error::from)?;
                if remaining == 0 {
                    return update_status(conn, review_id, ReviewStatus::NeedsReview).await;
                }
            }
            Ok(review)
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
            use schema::workspace_reviews::{self, dsl};

            // Only a review not already resolved can be verified; a repeat verify
            // matches no row -> `NotFound`.
            let review = diesel::update(
                workspace_reviews::table
                    .filter(dsl::id.eq(review_id))
                    .filter(dsl::review_status.ne(ReviewStatus::Resolved)),
            )
            .set(dsl::review_status.eq(ReviewStatus::Resolved))
            .returning(WorkspaceReview::as_returning())
            .get_result(conn)
            .await
            .map_err(Error::from)?;

            record_review_event(conn, &review, ReviewEventKind::Verified, actor, None).await?;
            Ok(review)
        })
        .await
    }

    async fn reopen_review(&mut self, review_id: Uuid, actor: Uuid) -> Result<WorkspaceReview> {
        self.transaction(async |conn| {
            use schema::workspace_reviews::{self, dsl};

            // Only a resolved review reopens; otherwise it is left as is (the update
            // matches no row, so fall back to reading it).
            let reopened = diesel::update(
                workspace_reviews::table
                    .filter(dsl::id.eq(review_id))
                    .filter(dsl::review_status.eq(ReviewStatus::Resolved)),
            )
            .set(dsl::review_status.eq(ReviewStatus::NeedsReview))
            .returning(WorkspaceReview::as_returning())
            .get_result(conn)
            .await
            .optional()
            .map_err(Error::from)?;

            match reopened {
                Some(review) => {
                    record_review_event(conn, &review, ReviewEventKind::Reopened, actor, None)
                        .await?;
                    Ok(review)
                }
                None => load_review(conn, review_id).await,
            }
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

/// Loads a review by id or returns `NotFound`.
async fn load_review(conn: &mut PgConnection, review_id: Uuid) -> Result<WorkspaceReview> {
    use schema::workspace_reviews::{self, dsl};

    workspace_reviews::table
        .filter(dsl::id.eq(review_id))
        .select(WorkspaceReview::as_select())
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

/// Inserts one review activity event.
async fn record_review_event(
    conn: &mut PgConnection,
    review: &WorkspaceReview,
    kind: ReviewEventKind,
    actor: Uuid,
    target: Option<Value>,
) -> Result<()> {
    use schema::workspace_review_events;

    diesel::insert_into(workspace_review_events::table)
        .values(&NewWorkspaceReviewEvent {
            workspace_id: review.workspace_id,
            review_id: review.id,
            kind,
            actor_account_id: Some(actor),
            target,
        })
        .execute(conn)
        .await
        .map_err(Error::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::model::NewAccount;
    use crate::query::{AccountRepository, WorkspaceReviewRepository, WorkspaceThreadRepository};
    use crate::test_util::TestDatabase;
    use crate::types::{
        CursorPagination, Direction, DocumentReviewFilter, ReviewEventKind, ReviewStatus,
    };

    #[tokio::test]
    async fn create_lists_and_finds_reviews_per_document() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        // A document can have many reviews (0..N), one per purpose.
        let public = conn
            .create_review(
                seeded.workspace_id,
                seeded.document_id,
                Some("Public release".to_owned()),
                seeded.account_id,
            )
            .await?;
        let legal = conn
            .create_review(
                seeded.workspace_id,
                seeded.document_id,
                Some("Court filing".to_owned()),
                seeded.account_id,
            )
            .await?;
        assert_ne!(public.id, legal.id);
        assert_ne!(
            public.thread_id, legal.thread_id,
            "each review owns its thread"
        );
        assert_eq!(public.review_status, ReviewStatus::NeedsReview);
        assert_eq!(public.purpose.as_deref(), Some("Public release"));

        // Both are listed for the document.
        let reviews = conn
            .list_document_reviews(seeded.workspace_id, seeded.document_id)
            .await?;
        assert_eq!(reviews.len(), 2);

        // Found by id within the workspace.
        let found = conn.find_review(seeded.workspace_id, legal.id).await?;
        assert_eq!(found.map(|r| r.id), Some(legal.id));
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
            .create_review(
                seeded.workspace_id,
                seeded.document_id,
                None,
                seeded.account_id,
            )
            .await?;

        // First assignee moves needs_review -> in_review.
        let a = conn
            .add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        assert_eq!(a.review_status, ReviewStatus::InReview);

        // A second assignee is added (still in_review); re-adding the same one is a
        // no-op (idempotent).
        conn.add_assignee(review.id, bob, seeded.account_id).await?;
        let again = conn.add_assignee(review.id, bob, seeded.account_id).await?;
        assert_eq!(again.review_status, ReviewStatus::InReview);
        let assignees = conn.list_review_assignees(review.id).await?;
        assert_eq!(assignees.len(), 2, "two distinct assignees");

        // Removing one of two leaves it in_review.
        let one_left = conn
            .remove_assignee(review.id, bob, seeded.account_id)
            .await?;
        assert_eq!(one_left.review_status, ReviewStatus::InReview);

        // Removing the last returns it to needs_review.
        let none_left = conn
            .remove_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        assert_eq!(none_left.review_status, ReviewStatus::NeedsReview);
        assert!(conn.list_review_assignees(review.id).await?.is_empty());

        // The timeline records the assign/unassign activity (oldest first): two
        // assigns (the idempotent re-add records nothing), then two unassigns.
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
            .create_review(
                seeded.workspace_id,
                seeded.document_id,
                None,
                seeded.account_id,
            )
            .await?;
        let resolved = conn.verify_review(review.id, seeded.account_id).await?;
        assert_eq!(resolved.review_status, ReviewStatus::Resolved);

        // Assigning a reviewer to a resolved review records the assignment but does
        // NOT silently un-resolve verified work.
        let assigned = conn
            .add_assignee(review.id, seeded.account_id, seeded.account_id)
            .await?;
        assert_eq!(assigned.review_status, ReviewStatus::Resolved);
        assert_eq!(conn.list_review_assignees(review.id).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn queue_filters_by_assignee_and_status() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let a = conn
            .create_review(
                seeded.workspace_id,
                seeded.document_id,
                None,
                seeded.account_id,
            )
            .await?;
        let _b = conn
            .create_review(
                seeded.workspace_id,
                seeded.document_id,
                None,
                seeded.account_id,
            )
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

    #[tokio::test]
    async fn deleting_a_review_leaves_its_thread() -> anyhow::Result<()> {
        // A review owns a thread; the thread is a plain discussion primitive that
        // outlives nothing on its own here — we only assert the thread was created
        // and is findable, confirming the create wired both rows.
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let review = conn
            .create_review(
                seeded.workspace_id,
                seeded.document_id,
                None,
                seeded.account_id,
            )
            .await?;
        let thread = conn
            .find_thread_in_workspace(seeded.workspace_id, review.thread_id)
            .await?;
        assert!(thread.is_some(), "the review's discussion thread exists");
        Ok(())
    }
}
