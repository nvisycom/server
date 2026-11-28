//! Review domain logic: opening a review, its discussion (comments, mentions, the
//! assistant), renaming, deletion, pointing at the detection under review, the
//! approval lifecycle (submit, request changes, approve, reopen), assignment, and
//! the review queue.
//!
//! A review is a named discussion on a document with an explicit approval lifecycle.
//! Its discussion and its approval workflow share one aggregate — mention
//! resolution, the assistant enqueue, the timeline, and the status all belong to
//! the same review — so one service owns them. It holds the assistant queue to wake
//! the reply drainer when a comment addresses the assistant.

use std::collections::BTreeSet;

use nvisy_postgres::model::{
    NewWorkspaceAssistantJob, NewWorkspaceReview, NewWorkspaceReviewComment,
    UpdateWorkspaceReviewComment, WorkspaceDocument, WorkspaceReview, WorkspaceReviewComment,
    WorkspaceReviewEvent,
};
use nvisy_postgres::query::{
    OutboxRepository, ReviewDetails, StatusOutcome, TimelineCursor, TimelineEntry,
    WorkspaceDetectionRepository, WorkspaceDocumentRepository, WorkspaceMemberRepository,
    WorkspaceRedactionRepository, WorkspaceRepository, WorkspaceReviewCommentRepository,
    WorkspaceReviewEventRepository, WorkspaceReviewRepository,
};
use nvisy_postgres::types::{
    AccountRefRow, CursorPage, CursorPagination, DocumentReviewFilter, Handle, ReviewSettings,
    ReviewStatus, TimeCursor, WithAccountRef,
};
use nvisy_postgres::{ASSISTANT_ACCOUNT_ID, ASSISTANT_HANDLE, AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::json::encode;
use crate::response::{Error, ErrorKind, Resource, Result};
use crate::service::event::EventEmitter;
use crate::service::{AssistantQueue, event};

/// Tracing target for review domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::review";

/// Manages reviews: opening, the discussion (comments, mentions, the assistant),
/// renaming, deletion, referencing artifacts, assignment, verification, and the
/// review queue.
///
/// Holds the Postgres client (acquiring its own connection per call) and the
/// assistant queue (to wake the reply drainer when a comment addresses the
/// assistant). Resolved per request from [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspaceReviewService {
    postgres: PgClient,
    assistant: AssistantQueue,
}

impl WorkspaceReviewService {
    /// Creates a [`WorkspaceReviewService`] over the connection pool and the
    /// assistant queue.
    #[must_use]
    pub fn new(postgres: PgClient, assistant: AssistantQueue) -> Self {
        Self {
            postgres,
            assistant,
        }
    }

    /// Opens a new review on a document with the given title, optionally against the
    /// detection it will review, recording the `review.opened` event (and
    /// the detection id in the opened event's target when a detection is given).
    ///
    /// A document may have many concurrent reviews (one per audience).
    ///
    /// # Errors
    ///
    /// - `NotFound` if the document, or the given detection, does not exist in the
    ///   workspace.
    /// - `BadRequest` if the detection is for a different document than the review.
    /// - A database error if the query fails.
    pub async fn open(
        &self,
        origin: event::EventOrigin<'_>,
        document_id: Uuid,
        detection_id: Option<Uuid>,
        display_name: String,
    ) -> Result<ReviewDetails> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let document = conn
            .find_document_in_workspace(workspace_id, document_id)
            .await?
            .ok_or_else(|| Error::not_found(Resource::Document))?;

        // A detection the review opens against must analyze the review's document.
        if let Some(detection_id) = detection_id {
            ensure_detection_on(&mut conn, workspace_id, detection_id, document_id).await?;
        }

        let review = conn
            .transaction(async |conn| {
                let review = conn
                    .create_review(NewWorkspaceReview {
                        workspace_id,
                        document_id,
                        detection_id,
                        author_account_id: origin.account_id,
                        display_name,
                    })
                    .await?;
                conn.emit_event(
                    origin,
                    event::ReviewOpened {
                        review_id: review.id,
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                    },
                )
                .await?;
                Ok::<_, Error>(conn.review_details(review).await?)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, review_id = %review.item.id, "Review opened");
        Ok(review)
    }

    /// Lists a workspace's reviews (the review queue), each paired with its
    /// assignees' account references.
    ///
    /// # Errors
    ///
    /// A database error if the query fails.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
        filter: &DocumentReviewFilter,
    ) -> Result<CursorPage<ReviewDetails>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_reviews(workspace_id, pagination, filter)
            .await?)
    }

    /// Lists a document's reviews.
    ///
    /// # Errors
    ///
    /// A database error if the query fails.
    pub async fn list_for_document(
        &self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> Result<Vec<ReviewDetails>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .list_document_reviews(workspace_id, document_id)
            .await?)
    }

    /// Finds a review by id.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn find(&self, workspace_id: Uuid, review_id: Uuid) -> Result<ReviewDetails> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, workspace_id, review_id).await?;
        Ok(conn.review_details(review).await?)
    }

    /// Lists a review's timeline: its comments and lifecycle events interleaved,
    /// oldest first.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn timeline(
        &self,
        workspace_id: Uuid,
        review_id: Uuid,
        pagination: CursorPagination<TimelineCursor>,
    ) -> Result<CursorPage<TimelineEntry>> {
        let mut conn = self.postgres.get_connection().await?;
        find_review(&mut conn, workspace_id, review_id).await?;
        Ok(conn.list_review_timeline(review_id, &pagination).await?)
    }

    /// Lists a review's lifecycle events, newest first.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn events(
        &self,
        workspace_id: Uuid,
        review_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceReviewEvent, Option<AccountRefRow>>>> {
        let mut conn = self.postgres.get_connection().await?;
        find_review(&mut conn, workspace_id, review_id).await?;
        Ok(conn
            .cursor_list_review_events(review_id, pagination)
            .await?)
    }

    /// Renames a review. `display_name` is `None` to leave the title unchanged
    /// (returned as-is), or `Some(name)` to set it (recording a rename event).
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn rename(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
        display_name: Option<String>,
    ) -> Result<ReviewDetails> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;

        // No name, or the current name, is a no-op: skip the write and the event.
        let Some(display_name) = display_name.filter(|name| *name != review.display_name) else {
            return Ok(conn.review_details(review).await?);
        };
        let document = review_document(&mut conn, &review).await?;

        let renamed = conn
            .transaction(async |conn| {
                let renamed = conn
                    .rename_review(review.id, display_name, origin.account_id)
                    .await?;
                conn.emit_event(
                    origin,
                    event::ReviewRenamed {
                        review_id: review.id,
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                    },
                )
                .await?;
                Ok::<_, Error>(conn.review_details(renamed).await?)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Review renamed");
        Ok(renamed)
    }

    /// Deletes a review and all of its comments (soft delete), recording the event
    /// atomically.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn delete(&self, origin: event::EventOrigin<'_>, review_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;

        conn.transaction(async |conn| {
            conn.delete_review(review.id).await?;
            conn.emit_event(
                origin,
                event::ReviewDeleted {
                    review_id: review.id,
                    document_id: review.document_id,
                },
            )
            .await?;
            Ok::<_, Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Review deleted");
        Ok(())
    }

    /// Posts a comment in a review, records its event, and — if the assistant was
    /// addressed — queues the reply job, all in one transaction. Rejects a comment
    /// on an approved review with a Conflict. Wakes the drainer after commit when a
    /// job was queued.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - `Conflict` if the review is approved.
    /// - `InternalServerError` if the queued assistant job cannot be encoded.
    /// - A database error if the query fails.
    pub async fn create_comment(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
        body: String,
    ) -> Result<WorkspaceReviewComment> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let author_id = origin.account_id;

        // The review must exist in the workspace (and be live).
        let review = find_review(&mut conn, workspace_id, review_id).await?;
        let mentions = resolve_mentions(&mut conn, workspace_id, &body, author_id).await?;

        let (comment, queued_assistant) = conn
            .transaction(async |conn| {
                // Lock the review and re-check its status inside the transaction: the
                // row lock serializes against a concurrent approve so a comment can
                // never land on a review after it is signed off.
                let locked = conn
                    .lock_review_in_workspace(workspace_id, review.id)
                    .await?
                    .ok_or_else(|| Error::not_found(Resource::Review))?;
                if locked.review_status == ReviewStatus::Approved {
                    return Err(ErrorKind::Conflict.with_message(
                        "This review is approved; reopen it before posting a comment",
                    ));
                }

                let comment = conn
                    .create_comment(NewWorkspaceReviewComment {
                        review_id: review.id,
                        author_account_id: author_id,
                        parent_id: None,
                        body,
                    })
                    .await?;
                conn.emit_event(
                    origin,
                    event::ReviewCommentCreated {
                        comment_id: comment.id,
                        review_id: review.id,
                        author_id,
                        mentioned: mentions.recipients,
                    },
                )
                .await?;
                let queued = enqueue_assistant_if_addressed(
                    conn,
                    mentions.addressed_assistant,
                    author_id,
                    workspace_id,
                    review.id,
                    comment.id,
                )
                .await?;
                Ok::<_, Error>((comment, queued))
            })
            .await?;

        if queued_assistant {
            self.assistant.wake_drainer();
        }

        tracing::info!(target: TRACING_TARGET, comment_id = %comment.id, "Comment posted");
        Ok(comment)
    }

    /// Edits a comment's body. Restricted to the comment's author.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the comment does not exist in the workspace.
    /// - `Forbidden` if the caller is not the comment's author.
    /// - A database error if the query fails.
    pub async fn update_comment(
        &self,
        workspace_id: Uuid,
        account_id: Uuid,
        comment_id: Uuid,
        body: String,
    ) -> Result<WorkspaceReviewComment> {
        let mut conn = self.postgres.get_connection().await?;
        let comment = find_comment(&mut conn, workspace_id, comment_id).await?;
        if comment.author_account_id != account_id {
            return Err(ErrorKind::Forbidden.with_message("Only the author can edit this comment"));
        }

        let updated = conn
            .update_comment_body(
                comment.id,
                UpdateWorkspaceReviewComment { body: Some(body) },
            )
            .await?;

        tracing::info!(target: TRACING_TARGET, "Comment edited");
        Ok(updated)
    }

    /// Soft-deletes a comment. Restricted to the comment's author.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the comment does not exist in the workspace.
    /// - `Forbidden` if the caller is not the comment's author.
    /// - A database error if the query fails.
    pub async fn delete_comment(
        &self,
        workspace_id: Uuid,
        account_id: Uuid,
        comment_id: Uuid,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let comment = find_comment(&mut conn, workspace_id, comment_id).await?;
        if comment.author_account_id != account_id {
            return Err(
                ErrorKind::Forbidden.with_message("Only the author can delete this comment")
            );
        }

        conn.delete_comment(comment.id).await?;
        tracing::info!(target: TRACING_TARGET, "Comment deleted");
        Ok(())
    }

    /// Points a review at the detection it is currently reviewing (its subject). Used
    /// on open and whenever the document is re-detected. Idempotent when the review
    /// already points at the detection.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review or the detection does not exist in the workspace.
    /// - `BadRequest` if the detection is for a different document than the review.
    /// - A database error if the query fails.
    pub async fn point_detection(
        &self,
        workspace_id: Uuid,
        review_id: Uuid,
        detection_id: Uuid,
        actor: Uuid,
    ) -> Result<ReviewDetails> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, workspace_id, review_id).await?;
        // A review's subject is always its own document's detection.
        ensure_detection_on(&mut conn, workspace_id, detection_id, review.document_id).await?;

        let review = conn.point_detection(review_id, detection_id, actor).await?;
        tracing::info!(target: TRACING_TARGET, "Review pointed at detection");
        Ok(conn.review_details(review).await?)
    }

    /// Submits (or resubmits) a review for review, moving it to `in_review` and
    /// raising the review-submitted event. Allowed from `draft` or
    /// `changes_requested`.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - `Conflict` if the review is not in a state that can be submitted.
    /// - A database error if the query fails.
    pub async fn submit(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
    ) -> Result<ReviewDetails> {
        self.transition(origin, review_id, Transition::Submit).await
    }

    /// Requests changes on a review, moving it to `changes_requested` (clearing any
    /// accumulated approvals) and raising the changes-requested event. Allowed only
    /// from `in_review` or `partially_approved`.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - `Conflict` if the review is not awaiting a decision.
    /// - A database error if the query fails.
    pub async fn request_changes(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
    ) -> Result<ReviewDetails> {
        self.transition(origin, review_id, Transition::RequestChanges)
            .await
    }

    /// Records the caller's approval of a review (by stamping their assignee row),
    /// optionally recording the approved redaction as its output. The review reaches
    /// `approved` once the number of approved assignees meets the workspace's
    /// `min_approvals`, otherwise it rests at `partially_approved`. Raises the
    /// review-approved event only when the quorum is reached. Honors the workspace's
    /// `author_can_approve` policy.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review, or the given redaction, does not exist in the
    ///   workspace.
    /// - `Conflict` if the review is not awaiting a decision.
    /// - `Forbidden` if the caller is not an assigned reviewer, or is the author
    ///   approving their own review where the workspace forbids it.
    /// - `BadRequest` if the redaction is for a different document than the review.
    /// - A database error if the query fails.
    pub async fn approve(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
        redaction_id: Option<Uuid>,
    ) -> Result<ReviewDetails> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let review = find_review(&mut conn, workspace_id, review_id).await?;
        if !matches!(
            review.review_status,
            ReviewStatus::InReview | ReviewStatus::PartiallyApproved
        ) {
            return Err(ErrorKind::Conflict
                .with_message("Only a review awaiting a decision can be approved"));
        }

        // A reviewer approves by stamping their own assignee row, so the caller must
        // be an assigned reviewer of this review.
        let assignees = conn.list_review_assignees(review_id).await?;
        if !assignees.iter().any(|a| a.id == origin.account_id) {
            return Err(ErrorKind::Forbidden
                .with_message("Only an assigned reviewer can approve this review"));
        }

        let policy = Self::review_settings(&mut conn, workspace_id).await?;
        if !policy.author_can_approve && review.author_account_id == origin.account_id {
            return Err(ErrorKind::Forbidden
                .with_message("The author of a review cannot approve it in this workspace"));
        }

        // The approved output, when given, must be a redaction of the review's own
        // document.
        if let Some(redaction_id) = redaction_id {
            let redaction = conn
                .find_redaction_in_workspace(workspace_id, redaction_id)
                .await?
                .ok_or_else(|| Error::not_found(Resource::Redaction))?;
            let detection = conn
                .find_workspace_detection_by_id(workspace_id, redaction.detection_id)
                .await?
                .ok_or_else(|| Error::not_found(Resource::Detection))?;
            if detection.input_document_id != review.document_id {
                return Err(ErrorKind::BadRequest
                    .with_message("Redaction is for a different document than this review"));
            }
        }

        let document = review_document(&mut conn, &review).await?;
        let required = policy.required_approvals();

        let updated = conn
            .transaction(async |conn| {
                let outcome = conn
                    .approve_review(review_id, origin.account_id, required, redaction_id)
                    .await?;
                // A later approval that names a different output than the one already
                // bound is rejected under the lock: the quorum must endorse one
                // redaction, so the approver either approves the bound one or reopens.
                if outcome.rebind_rejected {
                    return Err(ErrorKind::Conflict.with_message(
                        "This review already has an approved redaction; approve that one or reopen the review to change it",
                    ));
                }
                // Announce approval only when this call recorded a new approval that
                // reached quorum. `recorded` guards the lost-race case: if a concurrent
                // request-changes won between the unlocked pre-check and the row lock,
                // the repository returns the review unchanged with `recorded = false`,
                // and a quorum reached by some other request must not fire a second
                // event here. A partial approval advances state without an event.
                if outcome.recorded && outcome.review.review_status == ReviewStatus::Approved {
                    conn.emit_event(
                        origin,
                        event::ReviewApproved {
                            review_id: review.id,
                            document_id: document.id,
                            document_name: document.display_name.clone(),
                        },
                    )
                    .await?;
                }
                Ok::<_, Error>(conn.review_details(outcome.review).await?)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Review approval recorded");
        Ok(updated)
    }

    /// Reopens an approved review back to `in_review` (clearing its approvals and
    /// approved output).
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - `Conflict` if the review is not approved.
    /// - A database error if the query fails.
    pub async fn reopen(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
    ) -> Result<ReviewDetails> {
        self.transition(origin, review_id, Transition::Reopen).await
    }

    /// Moves a review through one of its status transitions: checks the current
    /// status allows it (a 409 otherwise), applies it under the row lock, and
    /// raises its event only when the status actually moved — a concurrent
    /// transition that won the race leaves it unchanged and must not be
    /// re-announced.
    async fn transition(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
        transition: Transition,
    ) -> Result<ReviewDetails> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;
        if !transition.allowed_from().contains(&review.review_status) {
            return Err(ErrorKind::Conflict.with_message(transition.conflict_message()));
        }
        let document = review_document(&mut conn, &review).await?;

        let updated = conn
            .transaction(async |conn| {
                let outcome = transition.apply(conn, review_id, origin.account_id).await?;
                if outcome.changed {
                    let event = transition.event(review.id, document.id, &document.display_name);
                    conn.emit_event(origin, event).await?;
                }
                Ok::<_, Error>(conn.review_details(outcome.review).await?)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, ?transition, "Review transitioned");
        Ok(updated)
    }

    /// Assigns a reviewer to a review (idempotent). The assignee must be a
    /// workspace member. Raises the review-assigned event, notifying the assignee
    /// unless they assigned themselves. Assignment is metadata (who is responsible);
    /// it does not change the review's status.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace, or the assignee
    ///   is not a workspace member.
    /// - A database error if the query fails.
    pub async fn add_assignee(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
        account_id: Uuid,
    ) -> Result<ReviewDetails> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;
        let document = review_document(&mut conn, &review).await?;

        // The assignee must be a workspace member (keeps a non-member from being
        // assigned across workspaces).
        conn.find_workspace_member_with_account(origin.workspace_id, account_id)
            .await?
            .ok_or_else(|| Error::not_found(Resource::Account))?;

        let actor_id = origin.account_id;
        let updated = conn
            .transaction(async |conn| {
                let outcome = conn.add_assignee(review_id, account_id, actor_id).await?;
                // Emit the workspace event only when the assignment actually changed
                // (a re-assign of an existing reviewer is a silent no-op, so no
                // spurious activity/webhook/notification).
                if outcome.changed {
                    conn.emit_event(
                        origin,
                        event::ReviewAssigned {
                            review_id: review.id,
                            document_id: document.id,
                            document_name: document.display_name.clone(),
                            assignee_id: account_id,
                            // The reviewer is notified unless they assigned themselves.
                            notify: (account_id != actor_id).then_some(account_id),
                        },
                    )
                    .await?;
                }
                Ok::<_, Error>(conn.review_details(outcome.review).await?)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Reviewer assigned");
        Ok(updated)
    }

    /// Removes a reviewer from a review. Raises the review-unassigned event. Removing
    /// a reviewer who had approved drops their approval, so a review that thereby falls
    /// below the workspace's required approvals is demoted (an approved review returns
    /// to in-review and drops its approved output).
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn remove_assignee(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
        account_id: Uuid,
    ) -> Result<ReviewDetails> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;
        let document = review_document(&mut conn, &review).await?;
        // Removing an approved assignee can drop the review below quorum, so the repo
        // reconciles the status under its lock against the workspace's required count.
        let required = Self::review_settings(&mut conn, origin.workspace_id)
            .await?
            .required_approvals();

        let updated = conn
            .transaction(async |conn| {
                let outcome = conn
                    .remove_assignee(review_id, account_id, origin.account_id, required)
                    .await?;
                // Emit only when a link was actually removed (removing a
                // non-assignee is a silent no-op).
                if outcome.changed {
                    conn.emit_event(
                        origin,
                        event::ReviewUnassigned {
                            review_id: review.id,
                            document_id: document.id,
                            document_name: Some(document.display_name.clone()),
                        },
                    )
                    .await?;
                }
                Ok::<_, Error>(conn.review_details(outcome.review).await?)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Reviewer unassigned");
        Ok(updated)
    }

    /// Loads a workspace's review approval policy from its settings, falling back to
    /// the defaults (single approver, author allowed) when the workspace or its
    /// settings cannot be read as expected.
    async fn review_settings(conn: &mut PgConn, workspace_id: Uuid) -> Result<ReviewSettings> {
        let workspace = conn
            .find_workspace_by_id(workspace_id)
            .await?
            .ok_or_else(|| Error::not_found(Resource::Workspace))?;
        Ok(workspace.settings.or_default().review)
    }
}

/// The outcome of resolving a comment body's `@`-mentions.
struct MentionOutcome {
    /// Workspace-member account ids to notify (de-duplicated, author excluded).
    recipients: Vec<Uuid>,
    /// Whether the body addressed the reserved assistant handle (`@assistant`), so
    /// an AI reply should be queued. The assistant is not a workspace member, so it
    /// never appears in `recipients` — it is a job trigger, not a notification
    /// target.
    addressed_assistant: bool,
}

/// Parses `@username` mentions from `body`. Resolves each human handle to a
/// workspace-member account id — de-duplicated, excluding `author`, and skipping
/// non-members — and separately reports whether the reserved assistant handle was
/// addressed.
async fn resolve_mentions(
    conn: &mut PgConn,
    workspace_id: Uuid,
    body: &str,
    author: Uuid,
) -> Result<MentionOutcome> {
    let raw: BTreeSet<String> = parse_mentions(body).into_iter().collect();
    let addressed_assistant = raw.iter().any(|m| m == ASSISTANT_HANDLE);

    let handles: Vec<Handle> = raw
        .into_iter()
        .filter_map(|m| Handle::parse(m).ok())
        .collect();
    if handles.is_empty() {
        return Ok(MentionOutcome {
            recipients: Vec::new(),
            addressed_assistant,
        });
    }

    let mut recipients = conn
        .find_member_ids_by_usernames(workspace_id, &handles)
        .await?;
    recipients.retain(|&id| id != author);
    Ok(MentionOutcome {
        recipients,
        addressed_assistant,
    })
}

/// Extracts the raw handle text of each `@username` mention in `body`.
///
/// A mention is an `@` that starts a token (preceded by start-of-string or a
/// non-alphanumeric, non-`@` char, so an email's `@` is not a mention) followed by
/// handle characters (`[a-z0-9-]`). Validation (length, dash rules) is left to
/// [`Handle::parse`]; this only slices candidate spans.
fn parse_mentions(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut mentions = Vec::new();
    let mut i = 0;
    while let Some(at) = body[i..].find('@') {
        let at = i + at;
        let boundary = at == 0 || {
            let prev = bytes[at - 1];
            !(prev.is_ascii_alphanumeric() || prev >= 0x80 || prev == b'@')
        };
        let start = at + 1;
        let end = start
            + body[start..]
                .find(|c: char| !matches!(c, 'a'..='z' | '0'..='9' | '-'))
                .unwrap_or(body.len() - start);
        if boundary && end > start {
            mentions.push(body[start..end].to_owned());
        }
        i = end.max(at + 1);
    }
    mentions
}

/// Queues an assistant-reply job for a just-created comment when it addressed the
/// assistant and was written by a human (not the assistant itself, so its own
/// replies never re-trigger it). Runs inside the comment's transaction so the job
/// commits atomically with the comment; returns whether a job was inserted, so the
/// caller can wake the drainer after commit.
async fn enqueue_assistant_if_addressed(
    conn: &mut PgConn,
    addressed_assistant: bool,
    author_id: Uuid,
    workspace_id: Uuid,
    review_id: Uuid,
    comment_id: Uuid,
) -> Result<bool> {
    use crate::worker::assistant::AssistantJob;

    if !addressed_assistant || author_id == ASSISTANT_ACCOUNT_ID {
        return Ok(false);
    }

    let job = AssistantJob {
        workspace_id,
        review_id,
        comment_id,
    };
    let payload = encode(&job, "assistant job")?;
    conn.insert_outbox(NewWorkspaceAssistantJob {
        comment_id,
        job: payload,
    })
    .await?;
    Ok(true)
}

/// Finds a review in the workspace or returns a 404.
async fn find_review(
    conn: &mut PgConn,
    workspace_id: Uuid,
    review_id: Uuid,
) -> Result<WorkspaceReview> {
    conn.find_review(workspace_id, review_id)
        .await?
        .ok_or_else(|| Error::not_found(Resource::Review))
}

/// Finds a live comment in the workspace or returns a 404.
async fn find_comment(
    conn: &mut PgConn,
    workspace_id: Uuid,
    comment_id: Uuid,
) -> Result<WorkspaceReviewComment> {
    conn.find_comment_in_workspace(workspace_id, comment_id)
        .await?
        .ok_or_else(|| Error::not_found(Resource::ReviewComment))
}

/// Loads the document a review is on (the review's `document_id` FK guarantees it
/// exists), for the document name carried by the review workspace events.
async fn review_document(conn: &mut PgConn, review: &WorkspaceReview) -> Result<WorkspaceDocument> {
    conn.find_document_in_workspace(review.workspace_id, review.document_id)
        .await?
        .ok_or_else(|| Error::not_found(Resource::Document))
}

/// Requires the detection to exist in the workspace and to be of `document_id`
/// (a review's subject is always its own document's detection).
async fn ensure_detection_on(
    conn: &mut PgConn,
    workspace_id: Uuid,
    detection_id: Uuid,
    document_id: Uuid,
) -> Result<()> {
    let detection = conn
        .find_workspace_detection_by_id(workspace_id, detection_id)
        .await?
        .ok_or_else(|| Error::not_found(Resource::Detection))?;
    if detection.input_document_id != document_id {
        return Err(ErrorKind::BadRequest
            .with_message("Detection is for a different document than this review"));
    }
    Ok(())
}

/// A review status transition driven by an explicit reviewer action.
#[derive(Debug, Clone, Copy)]
enum Transition {
    /// Draft or changes-requested → in review.
    Submit,
    /// Awaiting a decision → changes requested (clearing approvals).
    RequestChanges,
    /// Approved → in review (clearing approvals and the approved output).
    Reopen,
}

impl Transition {
    /// The statuses the transition may start from.
    fn allowed_from(self) -> &'static [ReviewStatus] {
        match self {
            Self::Submit => &[ReviewStatus::Draft, ReviewStatus::ChangesRequested],
            Self::RequestChanges => &[ReviewStatus::InReview, ReviewStatus::PartiallyApproved],
            Self::Reopen => &[ReviewStatus::Approved],
        }
    }

    /// The 409 message when the review is in any other status.
    fn conflict_message(self) -> &'static str {
        match self {
            Self::Submit => "Only a draft or changes-requested review can be submitted",
            Self::RequestChanges => "Only a review awaiting a decision can have changes requested",
            Self::Reopen => "Only an approved review can be reopened",
        }
    }

    /// Applies the transition under the review's row lock.
    async fn apply(self, conn: &mut PgConn, review_id: Uuid, actor: Uuid) -> Result<StatusOutcome> {
        Ok(match self {
            Self::Submit => conn.submit_review(review_id, actor).await?,
            Self::RequestChanges => conn.request_changes(review_id, actor).await?,
            Self::Reopen => conn.reopen_review(review_id, actor).await?,
        })
    }

    /// The event announcing the transition.
    fn event(
        self,
        review_id: Uuid,
        document_id: Uuid,
        document_name: &str,
    ) -> event::WorkspaceEvent {
        let document_name = document_name.to_owned();
        match self {
            Self::Submit => event::ReviewSubmitted {
                review_id,
                document_id,
                document_name,
            }
            .into(),
            Self::RequestChanges => event::ReviewChangesRequested {
                review_id,
                document_id,
                document_name,
            }
            .into(),
            Self::Reopen => event::ReviewReopened {
                review_id,
                document_id,
                document_name,
            }
            .into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_mentions;

    #[test]
    fn parses_mentions_and_ignores_emails() {
        assert_eq!(
            parse_mentions("@alice please review, cc @bob-smith — not user@example.com"),
            vec!["alice".to_owned(), "bob-smith".to_owned()],
        );
    }

    #[test]
    fn no_mentions_yields_empty() {
        assert_eq!(
            parse_mentions("just a plain comment, no pings"),
            Vec::<String>::new()
        );
        assert_eq!(parse_mentions(""), Vec::<String>::new());
        assert_eq!(parse_mentions("look @ this"), Vec::<String>::new());
    }

    #[test]
    fn mention_stops_at_non_handle_chars() {
        assert_eq!(parse_mentions("hey @carol!"), vec!["carol".to_owned()]);
        assert_eq!(parse_mentions("(@dave)"), vec!["dave".to_owned()]);
    }

    #[test]
    fn non_ascii_letter_before_at_is_not_a_boundary() {
        assert_eq!(parse_mentions("café@bob"), Vec::<String>::new());
        assert_eq!(parse_mentions("café @bob"), vec!["bob".to_owned()]);
    }
}
