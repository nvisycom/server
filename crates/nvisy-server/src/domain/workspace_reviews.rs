//! Review domain logic: opening a review, its discussion (comments, mentions, the
//! assistant), renaming, deletion, referencing detections and redactions,
//! assignment, verification, reopen, and the review queue.
//!
//! A review is a named discussion on a document with a manual sign-off lifecycle.
//! Its discussion and its sign-off workflow share one aggregate — mention
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
    AssistantJobOutboxRepository, DocumentReviewCursor, ReviewEventCursor, WithActor,
    WithReviewers, WorkspaceDetectionRepository, WorkspaceDocumentRepository,
    WorkspaceMemberRepository, WorkspaceRedactionRepository, WorkspaceReviewCommentRepository,
    WorkspaceReviewRepository,
};
use nvisy_postgres::types::{CursorPage, CursorPagination, DocumentReviewFilter, Handle};
use nvisy_postgres::{ASSISTANT_ACCOUNT_ID, ASSISTANT_HANDLE, AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::response::{Error, ErrorKind, Result};
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

    /// Opens a new review on a document with the given title, recording the
    /// `review.opened` event.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the document does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn open(
        &self,
        origin: event::EventOrigin<'_>,
        document_id: Uuid,
        display_name: String,
    ) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let document = conn
            .find_document_in_workspace(workspace_id, document_id)
            .await?
            .ok_or_else(|| Error::not_found("document"))?;

        let review = conn
            .transaction(async |conn| {
                let review = conn
                    .create_review(NewWorkspaceReview {
                        workspace_id,
                        document_id,
                        author_account_id: origin.account_id,
                        display_name,
                    })
                    .await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ReviewOpened(event::ReviewOpened {
                        review_id: review.id,
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                    }),
                )
                .await?;
                Ok::<_, Error>(review)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, review_id = %review.id, "Review opened");
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
        pagination: CursorPagination<DocumentReviewCursor>,
        filter: &DocumentReviewFilter,
    ) -> Result<CursorPage<WithReviewers<WorkspaceReview>>> {
        let mut conn = self.postgres.get_connection().await?;
        conn.cursor_list_reviews(workspace_id, pagination, filter)
            .await
            .map_err(Error::from)
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
    ) -> Result<Vec<WithReviewers<WorkspaceReview>>> {
        let mut conn = self.postgres.get_connection().await?;
        conn.list_document_reviews(workspace_id, document_id)
            .await
            .map_err(Error::from)
    }

    /// Finds a review by id.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn find(&self, workspace_id: Uuid, review_id: Uuid) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        find_review(&mut conn, workspace_id, review_id).await
    }

    /// Lists a review's activity timeline (its events only; the reader merges
    /// comments separately).
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn timeline(
        &self,
        workspace_id: Uuid,
        review_id: Uuid,
        pagination: CursorPagination<ReviewEventCursor>,
    ) -> Result<CursorPage<WithActor<WorkspaceReviewEvent>>> {
        let mut conn = self.postgres.get_connection().await?;
        find_review(&mut conn, workspace_id, review_id).await?;
        conn.cursor_list_review_events(review_id, pagination)
            .await
            .map_err(Error::from)
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
    ) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;

        let Some(display_name) = display_name else {
            return Ok(review);
        };
        // Renaming to the current name is a no-op: skip the write and the event.
        if review.display_name == display_name {
            return Ok(review);
        }
        let document = review_document(&mut conn, &review).await?;

        let renamed = conn
            .transaction(async |conn| {
                let renamed = conn
                    .rename_review(review.id, display_name, origin.account_id)
                    .await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ReviewRenamed(event::ReviewRenamed {
                        review_id: review.id,
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                    }),
                )
                .await?;
                Ok::<_, Error>(renamed)
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
                event::WorkspaceEvent::ReviewDeleted(event::ReviewDeleted {
                    review_id: review.id,
                    document_id: review.document_id,
                }),
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
    /// on a resolved review with a Conflict. Wakes the drainer after commit when a
    /// job was queued.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - `Conflict` if the review is resolved.
    /// - `InternalServerError` if the queued assistant job cannot be encoded.
    /// - A database error if the query fails.
    pub async fn create_comment(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
        body: String,
    ) -> Result<WorkspaceReviewComment> {
        use nvisy_postgres::types::ReviewStatus;

        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let author_id = origin.account_id;

        // The review must exist in the workspace (and be live).
        let review = find_review(&mut conn, workspace_id, review_id).await?;
        let mentions = resolve_mentions(&mut conn, workspace_id, &body, author_id).await?;

        let (comment, queued_assistant) = conn
            .transaction(async |conn| {
                // Lock the review and re-check its status inside the transaction: the
                // row lock serializes against a concurrent verify so a comment can
                // never land on a review after it resolves.
                let locked = conn
                    .lock_review_in_workspace(workspace_id, review.id)
                    .await?
                    .ok_or_else(|| Error::not_found("workspace_review"))?;
                if locked.review_status == ReviewStatus::Resolved {
                    return Err(ErrorKind::Conflict.with_message(
                        "This review is resolved; reopen it before posting a comment",
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
                    event::WorkspaceEvent::ReviewCommentCreated(event::ReviewCommentCreated {
                        comment_id: comment.id,
                        review_id: review.id,
                        author_id,
                        mentioned: mentions.recipients,
                    }),
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

    /// References a detection from a review.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review or the detection does not exist in the workspace.
    /// - `BadRequest` if the detection is for a different document than the review.
    /// - A database error if the query fails.
    pub async fn link_detection(
        &self,
        workspace_id: Uuid,
        review_id: Uuid,
        detection_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, workspace_id, review_id).await?;
        let (detection, _) = conn
            .find_workspace_detection_by_id(workspace_id, detection_id)
            .await?
            .ok_or_else(|| Error::not_found("workspace_detection"))?;

        // A review references only its own document's work: the detection must
        // analyze the review's document.
        if detection.input_document_id != review.document_id {
            return Err(ErrorKind::BadRequest
                .with_message("Detection is for a different document than this review"));
        }

        let review = conn.link_detection(review_id, detection_id, actor).await?;
        tracing::info!(target: TRACING_TARGET, "Detection linked to review");
        Ok(review)
    }

    /// References a redaction from a review.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review or the redaction does not exist in the workspace.
    /// - `BadRequest` if the redaction is for a different document than the review.
    /// - A database error if the query fails.
    pub async fn link_redaction(
        &self,
        workspace_id: Uuid,
        review_id: Uuid,
        redaction_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, workspace_id, review_id).await?;
        let redaction = conn
            .find_redaction_in_workspace(workspace_id, redaction_id)
            .await?
            .ok_or_else(|| Error::not_found("workspace_redaction"))?;
        // A redaction's document is its detection's input document; it must be the
        // review's document.
        let (detection, _) = conn
            .find_workspace_detection_by_id(workspace_id, redaction.detection_id)
            .await?
            .ok_or_else(|| Error::not_found("workspace_detection"))?;
        if detection.input_document_id != review.document_id {
            return Err(ErrorKind::BadRequest
                .with_message("Redaction is for a different document than this review"));
        }

        let review = conn.link_redaction(review_id, redaction_id, actor).await?;
        tracing::info!(target: TRACING_TARGET, "Redaction linked to review");
        Ok(review)
    }

    /// Verifies a review, moving it to `resolved`, and raises the review-verified
    /// event.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - `Conflict` if the review is already resolved.
    /// - A database error if the query fails.
    pub async fn verify(
        &self,
        origin: event::EventOrigin<'_>,
        review_id: Uuid,
    ) -> Result<WorkspaceReview> {
        use nvisy_postgres::types::ReviewStatus;

        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;
        if review.review_status == ReviewStatus::Resolved {
            return Err(ErrorKind::Conflict.with_message("This review is already resolved"));
        }
        let document = review_document(&mut conn, &review).await?;

        let verified = conn
            .transaction(async |conn| {
                let verified = conn.verify_review(review_id, origin.account_id).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ReviewVerified(event::ReviewVerified {
                        review_id: review.id,
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                    }),
                )
                .await?;
                Ok::<_, Error>(verified)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Review verified");
        Ok(verified)
    }

    /// Reopens a resolved review back to `needs_review`.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn reopen(
        &self,
        workspace_id: Uuid,
        review_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        find_review(&mut conn, workspace_id, review_id).await?;
        let review = conn.reopen_review(review_id, actor).await?;
        tracing::info!(target: TRACING_TARGET, "Review reopened");
        Ok(review)
    }

    /// Assigns a reviewer to a review (idempotent). The assignee must be a
    /// workspace member. Raises the review-assigned event, notifying the assignee
    /// unless they assigned themselves. The first assignee of a not-yet-resolved
    /// review moves it to `in_review`.
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
    ) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;
        let document = review_document(&mut conn, &review).await?;

        // The assignee must be a workspace member (keeps a non-member from being
        // assigned across workspaces).
        conn.find_workspace_member_with_account(origin.workspace_id, account_id)
            .await?
            .ok_or_else(|| Error::not_found("account"))?;

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
                        event::WorkspaceEvent::ReviewAssigned(event::ReviewAssigned {
                            review_id: review.id,
                            document_id: document.id,
                            document_name: document.display_name.clone(),
                            assignee_id: account_id,
                            // The reviewer is notified unless they assigned themselves.
                            notify: (account_id != actor_id).then_some(account_id),
                        }),
                    )
                    .await?;
                }
                Ok::<_, Error>(outcome.review)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Reviewer assigned");
        Ok(updated)
    }

    /// Removes a reviewer from a review. Raises the review-unassigned event. Clearing
    /// the last assignee of an in-review review returns it to `needs_review`.
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
    ) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        let review = find_review(&mut conn, origin.workspace_id, review_id).await?;
        let document = review_document(&mut conn, &review).await?;

        let updated = conn
            .transaction(async |conn| {
                let outcome = conn
                    .remove_assignee(review_id, account_id, origin.account_id)
                    .await?;
                // Emit only when a link was actually removed (removing a
                // non-assignee is a silent no-op).
                if outcome.changed {
                    conn.emit_event(
                        origin,
                        event::WorkspaceEvent::ReviewUnassigned(event::ReviewUnassigned {
                            review_id: review.id,
                            document_id: document.id,
                            document_name: Some(document.display_name.clone()),
                        }),
                    )
                    .await?;
                }
                Ok::<_, Error>(outcome.review)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Reviewer unassigned");
        Ok(updated)
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
    let payload = serde_json::to_value(&job).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to encode assistant job")
            .with_context(err.to_string())
    })?;
    conn.insert_assistant_job(NewWorkspaceAssistantJob {
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
        .ok_or_else(|| Error::not_found("workspace_review"))
}

/// Finds a live comment in the workspace or returns a 404.
async fn find_comment(
    conn: &mut PgConn,
    workspace_id: Uuid,
    comment_id: Uuid,
) -> Result<WorkspaceReviewComment> {
    conn.find_comment_in_workspace(workspace_id, comment_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_review_comment"))
}

/// Loads the document a review is on (the review's `document_id` FK guarantees it
/// exists), for the document name carried by the review workspace events.
async fn review_document(conn: &mut PgConn, review: &WorkspaceReview) -> Result<WorkspaceDocument> {
    conn.find_document_in_workspace(review.workspace_id, review.document_id)
        .await?
        .ok_or_else(|| Error::not_found("document"))
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
        assert!(parse_mentions("just a plain comment, no pings").is_empty());
        assert!(parse_mentions("").is_empty());
        assert!(parse_mentions("look @ this").is_empty());
    }

    #[test]
    fn mention_stops_at_non_handle_chars() {
        assert_eq!(parse_mentions("hey @carol!"), vec!["carol".to_owned()]);
        assert_eq!(parse_mentions("(@dave)"), vec!["dave".to_owned()]);
    }

    #[test]
    fn non_ascii_letter_before_at_is_not_a_boundary() {
        assert!(parse_mentions("café@bob").is_empty());
        assert_eq!(parse_mentions("café @bob"), vec!["bob".to_owned()]);
    }
}
