//! Document-review domain logic: opening a review, referencing detections and
//! redactions, assignment, verification, and the review queue. A review is an
//! optional, purpose-scoped sign-off effort on a document; it owns a discussion
//! thread ([`WorkspaceThreadService`](super::WorkspaceThreadService) handles the
//! discussion itself) and has its own activity log. This service drives the review
//! state and raises the `review.*` workspace events.

use nvisy_postgres::model::{WorkspaceDocument, WorkspaceReview, WorkspaceReviewEvent};
use nvisy_postgres::query::{
    DocumentReviewCursor, ReviewEventCursor, WithActor, WithReviewers,
    WorkspaceDetectionRepository, WorkspaceDocumentRepository, WorkspaceMemberRepository,
    WorkspaceRedactionRepository, WorkspaceReviewRepository,
};
use nvisy_postgres::types::{CursorPage, CursorPagination, DocumentReviewFilter};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::response::{Error, ErrorKind, Result};
use crate::service::event;
use crate::service::event::EventEmitter;

/// Tracing target for document-review domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::review";

/// Manages document reviews: opening, referencing artifacts, assignment,
/// verification, and the review queue.
///
/// Holds the Postgres client (acquiring its own connection per call). Resolved
/// per request from [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspaceReviewService {
    postgres: PgClient,
}

impl WorkspaceReviewService {
    /// Creates a [`WorkspaceReviewService`] over the connection pool.
    #[must_use]
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// Opens a new review on a document, with an optional purpose label.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the document does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn open(
        &self,
        workspace_id: Uuid,
        document_id: Uuid,
        purpose: Option<String>,
        actor: Uuid,
    ) -> Result<WorkspaceReview> {
        let mut conn = self.postgres.get_connection().await?;
        conn.find_document_in_workspace(workspace_id, document_id)
            .await?
            .ok_or_else(|| Error::not_found("document"))?;

        let review = conn
            .create_review(workspace_id, document_id, purpose, actor)
            .await?;

        tracing::info!(target: TRACING_TARGET, review_id = %review.id, "Review opened");
        Ok(review)
    }

    /// Lists a workspace's reviews (the review queue), each paired with its
    /// assignee's account reference.
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

    /// Lists a review's activity timeline.
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

    /// References a detection from a review.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the review or the detection does not exist in the workspace.
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
                        thread_id: review.thread_id,
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
                let updated = conn.add_assignee(review_id, account_id, actor_id).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ReviewAssigned(event::ReviewAssigned {
                        thread_id: review.thread_id,
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                        assignee_id: account_id,
                        // The reviewer is notified unless they assigned themselves.
                        notify: (account_id != actor_id).then_some(account_id),
                    }),
                )
                .await?;
                Ok::<_, Error>(updated)
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
                let updated = conn
                    .remove_assignee(review_id, account_id, origin.account_id)
                    .await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ReviewUnassigned(event::ReviewUnassigned {
                        thread_id: review.thread_id,
                        document_id: document.id,
                        document_name: Some(document.display_name.clone()),
                    }),
                )
                .await?;
                Ok::<_, Error>(updated)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Reviewer unassigned");
        Ok(updated)
    }
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

/// Loads the document a review is on (the review's `document_id` FK guarantees it
/// exists), for the document name carried by the review workspace events.
async fn review_document(conn: &mut PgConn, review: &WorkspaceReview) -> Result<WorkspaceDocument> {
    conn.find_document_in_workspace(review.workspace_id, review.document_id)
        .await?
        .ok_or_else(|| Error::not_found("document"))
}
