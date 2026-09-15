//! Document-review handlers: opening reviews, the review queue, a document's
//! reviews, referencing detections/redactions, the review timeline, and the review
//! transitions (verify, assign, reopen). A thin HTTP layer over
//! [`WorkspaceReviewService`].
//!
//! A review is an optional, purpose-scoped sign-off effort on a document (0..N per
//! document), opened explicitly by a reviewer. It owns a discussion thread and
//! references the detections/redactions done for its purpose. Opening, viewing,
//! and participating (`Review`); the queue and reads (`ViewReviews`); assigning
//! (`AssignReviews`).

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgConn;
use uuid::Uuid;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    AssignWorkspaceReview, CreateWorkspaceReview, CursorPagination, WorkspaceDocumentPathParams,
    WorkspaceReviewDetectionPathParams, WorkspaceReviewPathParams,
    WorkspaceReviewRedactionPathParams, WorkspaceReviewsQuery,
};
use crate::handler::response::{
    AccountRef, WorkspaceReview, WorkspaceReviewEvent, WorkspaceReviewTimelinePage,
    WorkspaceReviewsPage,
};
use crate::response::{ErrorResponse, Result};
use crate::service::{ServiceState, event};

/// Tracing target for document-review operations.
const TRACING_TARGET: &str = "nvisy_server::handler::reviews";

/// Opens a review on a document, with an optional `purpose` label.
///
/// A document may have any number of reviews (one per purpose/audience). Requires
/// `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
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

    let workspace = authz.workspace;
    let review = reviews
        .open(
            workspace.id,
            path_params.document_id,
            request.purpose,
            authz.account_id,
        )
        .await?;

    let response = WorkspaceReview::from_model(&review, None);
    Ok((StatusCode::CREATED, Json(response)))
}

fn open_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Open a document review")
        .description(
            "Opens a review on a document, with an optional purpose label. A document \
             may have many reviews (one per purpose). Requires Review.",
        )
        .response::<201, Json<WorkspaceReview>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists a document's reviews, most recent first. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        document_id = %path_params.document_id,
    )
)]
async fn list_document_reviews(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
) -> Result<(StatusCode, Json<Vec<WorkspaceReview>>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing document reviews");

    let workspace = authz.workspace;
    let rows = reviews
        .list_for_document(workspace.id, path_params.document_id)
        .await?;

    let items = rows
        .into_iter()
        .map(|row| WorkspaceReview::from_model(&row.item, row.assignee.map(AccountRef::from)))
        .collect();

    Ok((StatusCode::OK, Json(items)))
}

fn list_document_reviews_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List a document's reviews")
        .description("Returns a document's reviews, most recent first. Requires ViewReviews.")
        .response::<200, Json<Vec<WorkspaceReview>>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists a workspace's reviews (the review queue), most recent first.
///
/// Filter by `documentId`, `assignee`, and `reviewStatus` — an `assignee` +
/// `reviewStatus` pair is a reviewer's "needs review" queue. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_reviews(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceReviewsQuery>,
) -> Result<(StatusCode, Json<WorkspaceReviewsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing reviews");

    let workspace = authz.workspace;
    let page = reviews
        .list(workspace.id, pagination.into_cursor(), &query.into_filter())
        .await?;

    // The assignee reference rides along on each row (a left join), so no N+1.
    let items = page
        .items
        .into_iter()
        .map(|row| WorkspaceReview::from_model(&row.item, row.assignee.map(AccountRef::from)))
        .collect();

    let response = WorkspaceReviewsPage {
        items,
        total: page.total,
        next_cursor: page.next_cursor,
    };

    Ok((StatusCode::OK, Json(response)))
}

fn list_reviews_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List reviews")
        .description(
            "Returns the workspace's reviews, most recent first, with optional \
             document, assignee, and review-status filters (the review queue). \
             Requires ViewReviews.",
        )
        .response::<200, Json<WorkspaceReviewsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Returns a review by id. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        review_id = %path_params.review_id,
    )
)]
async fn get_review(
    State(pg_client): State<nvisy_postgres::PgClient>,
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
) -> Result<(StatusCode, Json<WorkspaceReview>)> {
    tracing::debug!(target: TRACING_TARGET, "Fetching review");

    let workspace = authz.workspace;
    let review = reviews.find(workspace.id, path_params.review_id).await?;

    let mut conn = pg_client.get_connection().await?;
    let response = review_response(&mut conn, review).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn get_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get a review")
        .description("Returns a review by id. Requires ViewReviews.")
        .response::<200, Json<WorkspaceReview>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists a review's activity timeline, oldest first. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        review_id = %path_params.review_id,
    )
)]
async fn list_review_timeline(
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::ViewReviews>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspaceReviewTimelinePage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing review timeline");

    // A timeline reads oldest-first.
    let workspace = authz.workspace;
    let page = reviews
        .timeline(
            workspace.id,
            path_params.review_id,
            pagination
                .into_cursor()
                .with_direction(nvisy_postgres::types::Direction::Ascending),
        )
        .await?;

    let items = page
        .items
        .into_iter()
        .map(|row| WorkspaceReviewEvent::from_model(row.item, row.actor.map(AccountRef::from)))
        .collect();

    let response = WorkspaceReviewTimelinePage {
        items,
        total: page.total,
        next_cursor: page.next_cursor,
    };

    Ok((StatusCode::OK, Json(response)))
}

fn list_review_timeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List a review's timeline")
        .description(
            "Returns a review's activity timeline (links, assignment, verification, \
             reopen), oldest first. Requires ViewReviews.",
        )
        .response::<200, Json<WorkspaceReviewTimelinePage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// References a detection from a review. Idempotent. Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        review_id = %path_params.review_id,
        detection_id = %path_params.detection_id,
    )
)]
async fn link_detection(
    State(pg_client): State<nvisy_postgres::PgClient>,
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewDetectionPathParams>,
) -> Result<(StatusCode, Json<WorkspaceReview>)> {
    tracing::debug!(target: TRACING_TARGET, "Linking detection to review");

    let workspace = authz.workspace;
    let review = reviews
        .link_detection(
            workspace.id,
            path_params.review_id,
            path_params.detection_id,
            authz.account_id,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = review_response(&mut conn, review).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn link_detection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reference a detection from a review")
        .description("Links a detection to a review (idempotent). Requires Review.")
        .response::<200, Json<WorkspaceReview>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// References a redaction from a review. Idempotent. Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        review_id = %path_params.review_id,
        redaction_id = %path_params.redaction_id,
    )
)]
async fn link_redaction(
    State(pg_client): State<nvisy_postgres::PgClient>,
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewRedactionPathParams>,
) -> Result<(StatusCode, Json<WorkspaceReview>)> {
    tracing::debug!(target: TRACING_TARGET, "Linking redaction to review");

    let workspace = authz.workspace;
    let review = reviews
        .link_redaction(
            workspace.id,
            path_params.review_id,
            path_params.redaction_id,
            authz.account_id,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = review_response(&mut conn, review).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn link_redaction_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reference a redaction from a review")
        .description("Links a redaction to a review (idempotent). Requires Review.")
        .response::<200, Json<WorkspaceReview>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Verifies a review, moving it to `resolved`. Requires `Review` (a reviewer signs
/// off their own work).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        review_id = %path_params.review_id,
    )
)]
async fn verify_review(
    State(pg_client): State<nvisy_postgres::PgClient>,
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<WorkspaceReview>)> {
    tracing::debug!(target: TRACING_TARGET, "Verifying review");

    let workspace = authz.workspace;
    let verified = reviews
        .verify(
            origin(workspace.id, authz.account_id, &security),
            path_params.review_id,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = review_response(&mut conn, verified).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn verify_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Verify a review")
        .description("Verifies a review, moving it to `resolved`. Requires Review.")
        .response::<200, Json<WorkspaceReview>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Reopens a resolved review back to `needs_review`. Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        review_id = %path_params.review_id,
    )
)]
async fn reopen_review(
    State(pg_client): State<nvisy_postgres::PgClient>,
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
) -> Result<(StatusCode, Json<WorkspaceReview>)> {
    tracing::debug!(target: TRACING_TARGET, "Reopening review");

    let workspace = authz.workspace;
    let reopened = reviews
        .reopen(workspace.id, path_params.review_id, authz.account_id)
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = review_response(&mut conn, reopened).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn reopen_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reopen a review")
        .description("Reopens a resolved review back to `needs_review`. Requires Review.")
        .response::<200, Json<WorkspaceReview>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Assigns or unassigns a review.
///
/// A `null` assignee clears the current one. An assignee must be a workspace
/// member. Requires `AssignReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        review_id = %path_params.review_id,
    )
)]
async fn assign_review(
    State(pg_client): State<nvisy_postgres::PgClient>,
    State(reviews): State<domain::WorkspaceReviewService>,
    authz: Authorized<markers::AssignReviews>,
    Path(path_params): Path<WorkspaceReviewPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<AssignWorkspaceReview>,
) -> Result<(StatusCode, Json<WorkspaceReview>)> {
    tracing::debug!(target: TRACING_TARGET, "Assigning review");

    let workspace = authz.workspace;
    let updated = reviews
        .assign(
            origin(workspace.id, authz.account_id, &security),
            path_params.review_id,
            request.assignee,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = review_response(&mut conn, updated).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn assign_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Assign a review")
        .description(
            "Assigns a review to a workspace member, or clears the assignee with a null \
             `assignee`. Requires AssignReviews.",
        )
        .response::<200, Json<WorkspaceReview>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Builds the event origin shared by every review event.
fn origin(
    workspace_id: Uuid,
    account_id: Uuid,
    security: &SecurityContext,
) -> event::EventOrigin<'_> {
    event::EventOrigin {
        workspace_id,
        account_id,
        security,
    }
}

/// Builds a full [`WorkspaceReview`] response, resolving its assignee.
async fn review_response(
    conn: &mut PgConn,
    review: nvisy_postgres::model::WorkspaceReview,
) -> Result<WorkspaceReview> {
    use crate::handler::utility::resolve_account_ref_opt;
    let assignee = resolve_account_ref_opt(conn, review.assignee_account_id).await?;
    Ok(WorkspaceReview::from_model(&review, assignee))
}

/// Returns an [`ApiRouter`] with the document-review routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with, put_with};

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
            get_with(get_review, get_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/timeline",
            get_with(list_review_timeline, list_review_timeline_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/verify",
            post_with(verify_review, verify_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/reopen",
            post_with(reopen_review, reopen_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/assign",
            put_with(assign_review, assign_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/detections/{detectionId}",
            post_with(link_detection, link_detection_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/reviews/{reviewId}/redactions/{redactionId}",
            post_with(link_redaction, link_redaction_docs),
        )
        .with_path_items(|item| item.tag("DocumentReviews"))
}
