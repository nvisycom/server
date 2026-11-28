//! Review handlers: opening reviews, the review queue, a document's reviews, the
//! discussion (comments), the interleaved timeline, pointing at the detection under
//! review, and the review transitions (rename, delete, submit, request-changes,
//! approve, reopen, assign). A thin HTTP layer over [`WorkspaceReviewService`].
//!
//! A review is a named discussion on a document with an explicit approval lifecycle
//! (0..N per document, concurrent per audience), opened explicitly by a reviewer. It is
//! anchored to its document and points at the detection it reviews and the redaction
//! it approves. Viewing and reads (`ViewReviews`); opening, commenting, pointing,
//! and the approval transitions (`Review`); renaming and deleting (`ManageReviews`);
//! assigning (`AssignReviews`).

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::query::{ReviewDetails, TimelineCursor};
use nvisy_postgres::types::Direction;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceComment, CreateWorkspaceReview, CursorPagination, RenameWorkspaceReview,
    UpdateWorkspaceComment, WorkspaceCommentPathParams, WorkspaceDocumentPathParams,
    WorkspaceReviewAssigneePathParams, WorkspaceReviewDetectionPathParams,
    WorkspaceReviewPathParams, WorkspaceReviewsQuery,
};
use crate::handler::response::{
    AccountRef, WorkspaceComment, WorkspaceReview, WorkspaceReviewEntry, WorkspaceReviewEvent,
    WorkspaceReviewTimelinePage, WorkspaceReviewsPage, WorkspaceTimelinePage,
};
use crate::response::{ErrorResponse, Result};
use crate::service::ServiceState;

/// Tracing target for review operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_reviews";

/// Opens a review on a document, with a `displayName` title.
///
/// A document may have any number of reviews. Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        document_id = %path_params.document_id,
    )
)]
async fn open_review(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
    ValidateJson(request): ValidateJson<CreateWorkspaceReview>,
) -> Result<(StatusCode, Json<WorkspaceReview>)> {
    tracing::debug!(target: TRACING_TARGET, "Opening review");

    let review = reviews
        .open(
            authz.origin(),
            path_params.document_id,
            request.detection_id.map(|id| id.as_uuid()),
            request.display_name,
        )
        .await?;

    let response = review_details_response(review);
    Ok((StatusCode::CREATED, Json(response)))
}

fn open_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Open a review")
        .description(
            "Opens a review on a document, with a title. A document may have many \
             reviews. Requires Review.",
        )
        .response::<201, Json<WorkspaceReview>>()
}

/// Lists a document's reviews, most recent first. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        document_id = %path_params.document_id,
    )
)]
async fn list_document_reviews(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
) -> Result<Json<Vec<WorkspaceReview>>> {
    tracing::debug!(target: TRACING_TARGET, "Listing document reviews");

    let workspace = &authz.workspace;
    let rows = reviews
        .list_for_document(workspace.id, path_params.document_id)
        .await?;

    let items = rows.into_iter().map(review_details_response).collect();

    Ok(Json(items))
}

fn list_document_reviews_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List a document's reviews")
        .description("Returns a document's reviews, most recent first. Requires ViewReviews.")
        .response::<200, Json<Vec<WorkspaceReview>>>()
}

/// Lists a workspace's reviews (the review queue), most recent first.
///
/// Filter by `documentId`, `author`, `assignee`, and `reviewStatus` — an
/// `assignee` + `reviewStatus` pair is a reviewer's queue. Requires `ViewReviews`.
#[tracing::instrument(skip_all)]
async fn list_reviews(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceReviewsQuery>,
) -> Result<Json<WorkspaceReviewsPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing reviews");

    let workspace = &authz.workspace;
    let page = reviews
        .list(workspace.id, pagination.into_cursor(), &query.into_filter())
        .await?;

    Ok(Json(WorkspaceReviewsPage::from_cursor_page(
        page,
        review_details_response,
    )))
}

fn list_reviews_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List reviews")
        .description(
            "Returns the workspace's reviews, most recent first, with optional \
             document, author, assignee, and review-status filters (the review \
             queue). Requires ViewReviews.",
        )
        .response::<200, Json<WorkspaceReviewsPage>>()
}

/// Returns a review by id. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn get_review(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Fetching review");

    let workspace = &authz.workspace;
    let review = reviews.find(workspace.id, path_params.review_id).await?;

    let response = review_details_response(review);

    Ok(Json(response))
}

fn get_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get a review")
        .description("Returns a review by id. Requires ViewReviews.")
        .response::<200, Json<WorkspaceReview>>()
}

/// Renames a review (sets its title). Requires `ManageReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn rename_review(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ManageReviews>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
    ValidateJson(request): ValidateJson<RenameWorkspaceReview>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Renaming review");

    let renamed = reviews
        .rename(authz.origin(), path_params.review_id, request.display_name)
        .await?;

    let response = review_details_response(renamed);

    Ok(Json(response))
}

fn rename_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Rename a review")
        .description("Sets a review's title. Requires ManageReviews.")
        .response::<200, Json<WorkspaceReview>>()
}

/// Deletes a review and all of its comments (soft delete). Requires
/// `ManageReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn delete_review(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ManageReviews>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting review");

    reviews
        .delete(authz.origin(), path_params.review_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn delete_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete a review")
        .description("Soft-deletes a review and all of its comments. Requires ManageReviews.")
        .response::<204, ()>()
}

/// Posts a comment (message) in a review.
///
/// `@username` mentions in the body notify those workspace members. Requires
/// `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn create_comment(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
    ValidateJson(request): ValidateJson<CreateWorkspaceComment>,
) -> Result<(StatusCode, Json<WorkspaceComment>)> {
    tracing::debug!(target: TRACING_TARGET, "Posting comment");

    let comment = reviews
        .create_comment(authz.origin(), path_params.review_id, request.body)
        .await?;

    // The author is the caller.
    let author = authz.account.clone().into();

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceComment::from_model(comment, author)),
    ))
}

fn create_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Post a comment")
        .description(
            "Posts a comment (message) in a review. @username mentions notify those \
             members. Requires the Review permission. Returns 409 if the review is \
             approved.",
        )
        .response::<201, Json<WorkspaceComment>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Edits a comment's body. Restricted to the comment's author.
#[tracing::instrument(
    skip_all,
    fields(
        comment_id = %path_params.comment_id,
    )
)]
async fn update_comment(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceCommentPathParams>,
    ValidateJson(request): ValidateJson<UpdateWorkspaceComment>,
) -> Result<Json<WorkspaceComment>> {
    tracing::debug!(target: TRACING_TARGET, "Editing comment");

    let workspace = &authz.workspace;
    let updated = reviews
        .update_comment(
            workspace.id,
            authz.account_id,
            path_params.comment_id,
            request.body,
        )
        .await?;

    // Only the author can edit a comment, so the author is the caller.
    Ok(Json(WorkspaceComment::from_model(
        updated,
        authz.account.clone().into(),
    )))
}

fn update_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Edit a comment")
        .description("Edits a comment's body. Only the author may edit their own comment.")
        .response::<200, Json<WorkspaceComment>>()
}

/// Deletes a comment (soft delete). Restricted to the comment's author.
#[tracing::instrument(
    skip_all,
    fields(
        comment_id = %path_params.comment_id,
    )
)]
async fn delete_comment(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceCommentPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting comment");

    let workspace = &authz.workspace;
    reviews
        .delete_comment(workspace.id, authz.account_id, path_params.comment_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn delete_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete a comment")
        .description("Soft-deletes a comment. Only the author may delete their own comment.")
        .response::<204, ()>()
}

/// Returns a review's full timeline: comments and lifecycle events interleaved,
/// oldest first. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn list_review_timeline(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<Json<WorkspaceTimelinePage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing review timeline");

    // The timeline reads oldest first, so it walks ascending.
    let pagination = pagination
        .into_cursor::<TimelineCursor>()
        .with_direction(Direction::Ascending);
    let page = reviews
        .timeline(authz.workspace.id, path_params.review_id, pagination)
        .await?;
    Ok(Json(WorkspaceTimelinePage::from_cursor_page(
        page,
        WorkspaceReviewEntry::from,
    )))
}

fn list_review_timeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List a review's timeline")
        .description(
            "Returns the review's timeline — comments and lifecycle events (opened, \
             renamed, redetected, assigned, unassigned, submitted, changes \
             requested, approved, reopened) interleaved, oldest first, with cursor \
             pagination. Requires ViewReviews.",
        )
        .response::<200, Json<WorkspaceTimelinePage>>()
}

/// Returns a review's activity events only (no comments), oldest first. Requires
/// `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn list_review_events(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<Json<WorkspaceReviewTimelinePage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing review events");

    let workspace = &authz.workspace;
    let page = reviews
        .events(
            workspace.id,
            path_params.review_id,
            pagination
                .into_cursor()
                .with_direction(Direction::Ascending),
        )
        .await?;

    Ok(Json(WorkspaceReviewTimelinePage::from_cursor_page(
        page,
        |row| WorkspaceReviewEvent::from_model(row.item, row.account.map(AccountRef::from)),
    )))
}

fn list_review_events_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List a review's events")
        .description(
            "Returns a review's activity events only (re-detection, assignment, \
             submission, change requests, approvals, reopen), oldest first. Requires \
             ViewReviews.",
        )
        .response::<200, Json<WorkspaceReviewTimelinePage>>()
}

/// Points a review at the detection it is currently reviewing. Idempotent. Used on
/// open and when the document is re-detected. Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
        detection_id = %path_params.detection_id,
    )
)]
async fn point_detection(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewDetectionPathParams>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Pointing review at detection");

    let workspace = &authz.workspace;
    let review = reviews
        .point_detection(
            workspace.id,
            path_params.review_id,
            path_params.detection_id.as_uuid(),
            authz.account_id,
        )
        .await?;

    let response = review_details_response(review);

    Ok(Json(response))
}

fn point_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Point a review at a detection")
        .description(
            "Points a review at the detection it is currently reviewing (idempotent). \
             Re-detecting a document repoints the review. Requires Review.",
        )
        .response::<200, Json<WorkspaceReview>>()
}

/// Submits (or resubmits) a review for review, moving it to `in_review`. Requires
/// `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn submit_review(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Submitting review");

    let submitted = reviews
        .submit(authz.origin(), path_params.review_id)
        .await?;

    let response = review_details_response(submitted);

    Ok(Json(response))
}

fn submit_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Submit a review")
        .description(
            "Submits (or resubmits) a review for review, moving it to `in_review`. \
             409 unless it is a draft or has changes requested. Requires Review.",
        )
        .response::<200, Json<WorkspaceReview>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Requests changes on a review, moving it to `changes_requested`. Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn request_changes(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Requesting changes on review");

    let updated = reviews
        .request_changes(authz.origin(), path_params.review_id)
        .await?;

    let response = review_details_response(updated);

    Ok(Json(response))
}

fn request_changes_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Request changes on a review")
        .description(
            "Requests changes on a review, moving it to `changes_requested` and \
             clearing any accumulated approvals. 409 unless it is awaiting a \
             decision. Requires Review.",
        )
        .response::<200, Json<WorkspaceReview>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Approves a review, recording the caller's approval and optionally its output
/// redaction. Reaches `approved` once the workspace's required approvals are in.
/// Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn approve_review(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
    ValidateJson(request): ValidateJson<crate::handler::request::ApproveWorkspaceReview>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Approving review");

    let approved = reviews
        .approve(
            authz.origin(),
            path_params.review_id,
            request.redaction_id.map(|id| id.as_uuid()),
        )
        .await?;

    let response = review_details_response(approved);

    Ok(Json(response))
}

fn approve_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Approve a review")
        .description(
            "Records the caller's approval and, optionally, the approved output \
             redaction. The review reaches `approved` once the workspace's required \
             approvals are in, otherwise `partially_approved`. 409 unless it is \
             awaiting a decision; 403 if the author approves their own review where \
             the workspace forbids it. Requires Review.",
        )
        .response::<200, Json<WorkspaceReview>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Reopens an approved review back to `in_review`. Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
    )
)]
async fn reopen_review(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Reopening review");

    let reopened = reviews
        .reopen(authz.origin(), path_params.review_id)
        .await?;

    let response = review_details_response(reopened);

    Ok(Json(response))
}

fn reopen_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reopen a review")
        .description(
            "Reopens an approved review back to `in_review`, clearing its approvals \
             and approved output. 409 unless it is approved. Requires Review.",
        )
        .response::<200, Json<WorkspaceReview>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Assigns a reviewer to a review (idempotent).
///
/// The account must be a workspace member. Requires `AssignReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
        assignee_id = %path_params.account_id,
    )
)]
async fn add_review_assignee(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::AssignReviews>,
    Path(path_params): Path<WorkspaceReviewAssigneePathParams>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Assigning reviewer");

    let updated = reviews
        .add_assignee(
            authz.origin(),
            path_params.review_id,
            path_params.account_id,
        )
        .await?;

    let response = review_details_response(updated);

    Ok(Json(response))
}

fn add_review_assignee_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Assign a reviewer")
        .description(
            "Assigns a workspace member as a reviewer of a review (idempotent). \
             Assignment is metadata (who is responsible) and does not change the \
             review's status. Requires AssignReviews.",
        )
        .response::<200, Json<WorkspaceReview>>()
}

/// Removes a reviewer from a review.
///
/// Requires `AssignReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        review_id = %path_params.review_id,
        assignee_id = %path_params.account_id,
    )
)]
async fn remove_review_assignee(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::AssignReviews>,
    Path(path_params): Path<WorkspaceReviewAssigneePathParams>,
) -> Result<Json<WorkspaceReview>> {
    tracing::debug!(target: TRACING_TARGET, "Unassigning reviewer");

    let updated = reviews
        .remove_assignee(
            authz.origin(),
            path_params.review_id,
            path_params.account_id,
        )
        .await?;

    let response = review_details_response(updated);

    Ok(Json(response))
}

fn remove_review_assignee_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Unassign a reviewer")
        .description(
            "Removes a reviewer from a review. Assignment is metadata and does not \
             change the review's status. Requires AssignReviews.",
        )
        .response::<200, Json<WorkspaceReview>>()
}

/// Maps a review with its people to its response.
fn review_details_response(details: ReviewDetails) -> WorkspaceReview {
    let assignees = details
        .assignees
        .into_iter()
        .map(AccountRef::from)
        .collect();
    WorkspaceReview::from_model(&details.item, details.author.into(), assignees)
}

/// Returns an [`ApiRouter`] with the review routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, patch_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/reviews",
            get_with(list_reviews, list_reviews_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/documents/{documentId}/reviews",
            post_with(open_review, open_review_docs)
                .get_with(list_document_reviews, list_document_reviews_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}",
            get_with(get_review, get_review_docs)
                .patch_with(rename_review, rename_review_docs)
                .delete_with(delete_review, delete_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/comments",
            post_with(create_comment, create_comment_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/comments/{commentId}",
            patch_with(update_comment, update_comment_docs)
                .delete_with(delete_comment, delete_comment_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/timeline",
            get_with(list_review_timeline, list_review_timeline_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/events",
            get_with(list_review_events, list_review_events_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/submit",
            post_with(submit_review, submit_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/request-changes",
            post_with(request_changes, request_changes_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/approve",
            post_with(approve_review, approve_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/reopen",
            post_with(reopen_review, reopen_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/assignees/{accountId}",
            post_with(add_review_assignee, add_review_assignee_docs)
                .delete_with(remove_review_assignee, remove_review_assignee_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/detections/{detectionId}",
            post_with(point_detection, point_detection_docs),
        )
        .with_path_items(|item| item.tag("Reviews"))
}
