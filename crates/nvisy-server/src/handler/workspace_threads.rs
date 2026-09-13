//! Thread handlers: the thread lifecycle and its GitHub-issue-style timeline.
//! The messages within a thread are handled by the sibling
//! `workspace_thread_comments` module. Both are a thin HTTP layer over
//! [`WorkspaceThreadService`], which owns the lifecycle, review transitions,
//! mention resolution, and assistant enqueue.
//!
//! A thread is one of two things. A *workspace thread* is free-form discussion,
//! opened by a member with a first message and closable, reopenable, renamable,
//! or deletable as a whole. A *document thread* is a document's review: exactly
//! one live thread per document, auto-created on the document's first detection
//! (never opened by hand), carrying an optional assignee and a `review_status`
//! derived from the review timeline. Viewing (`ViewReviews`) and participating —
//! commenting and verifying a review (`Review`) — are Reviewer-tier;
//! closing/reopening a workspace thread (`ManageThreads`) and assigning a review
//! (`AssignReviews`) are Editor-tier.

use std::collections::{BTreeSet, HashMap};

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::query::{
    AccountRepository, TimelineCursor, WorkspaceThreadCommentRepository,
    WorkspaceThreadEventRepository, WorkspaceThreadRepository,
};
use nvisy_postgres::types::{CursorPage, Direction};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    AssignWorkspaceReview, CursorPagination, OpenWorkspaceThread, RenameWorkspaceThread,
    WorkspaceDocumentPathParams, WorkspaceThreadPathParams, WorkspaceThreadsQuery,
};
use crate::handler::response::{
    AccountRef, WorkspaceComment, WorkspaceThread, WorkspaceThreadEntry, WorkspaceThreadEvent,
    WorkspaceThreadsPage, WorkspaceTimelinePage,
};
use crate::handler::utility::{resolve_account_ref, resolve_account_ref_opt};
use crate::response::{Error, ErrorResponse, Result};
use crate::service::{ServiceState, event};

/// Tracing target for thread operations.
pub(crate) const TRACING_TARGET: &str = "nvisy_server::handler::threads";

/// Opens a workspace-level discussion thread (not tied to any document), with its
/// first message.
///
/// Document reviews are auto-created on detection, not opened by hand, so this is
/// the only open endpoint. `@username` mentions in the opening body notify those
/// workspace members. Requires `Review`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn open_workspace_thread(
    State(pg_client): State<PgClient>,
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::Review>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<OpenWorkspaceThread>,
) -> Result<(StatusCode, Json<WorkspaceThread>)> {
    tracing::debug!(target: TRACING_TARGET, "Opening workspace thread");

    let workspace = authz.workspace;
    let thread = threads
        .open(
            origin(workspace.id, authz.account_id, &security),
            request.into(),
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = thread_response(&mut conn, thread).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

fn open_workspace_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Open a workspace thread")
        .description(
            "Opens a workspace-level discussion thread (not tied to any document) with \
             its first message. @username mentions notify those members. Requires \
             the Review permission.",
        )
        .response::<201, Json<WorkspaceThread>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists a workspace's threads with cursor pagination, most recent first.
///
/// Filter by `documentId`, `author`, and `closed`. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_threads(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewReviews>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceThreadsQuery>,
) -> Result<(StatusCode, Json<WorkspaceThreadsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing threads");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_threads(workspace.id, pagination.into_cursor(), &query.into_filter())
        .await?;

    // Resolve the page's distinct assignees (document reviews) in one query, so
    // each thread response can carry its assignee reference without an N+1 lookup.
    let assignee_ids: BTreeSet<Uuid> = page
        .items
        .iter()
        .filter_map(|row| row.item.assignee_account_id)
        .collect();
    let assignee_ids: Vec<Uuid> = assignee_ids.into_iter().collect();
    let assignees: HashMap<Uuid, AccountRef> = conn
        .find_accounts_by_ids(&assignee_ids)
        .await?
        .into_iter()
        .map(|account| {
            (
                account.id,
                AccountRef::new(
                    account.id,
                    account.username,
                    account.display_name,
                    account.avatar_url,
                ),
            )
        })
        .collect();

    let threads = page
        .items
        .into_iter()
        .map(|row| {
            let assignee = row
                .item
                .assignee_account_id
                .and_then(|id| assignees.get(&id).cloned());
            WorkspaceThread::from_model(row.item, row.account.into(), assignee)
        })
        .collect();

    let response = WorkspaceThreadsPage {
        items: threads,
        total: page.total,
        next_cursor: page.next_cursor,
    };

    Ok((StatusCode::OK, Json(response)))
}

fn list_threads_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List threads")
        .description(
            "Returns the workspace's threads, most recent first, with optional \
             document, author, assignee, review-status, and open/closed filters. \
             Pass `assignedToMe=true` for the caller's own review assignments \
             (it takes precedence over an explicit `assignee`).",
        )
        .response::<200, Json<WorkspaceThreadsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Deletes a thread and all of its comments (soft delete). Requires
/// `ManageThreads`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        thread_id = %path_params.thread_id,
    )
)]
async fn delete_thread(
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::ManageThreads>,
    Path(path_params): Path<WorkspaceThreadPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting thread");

    let workspace = authz.workspace;
    threads
        .delete(
            origin(workspace.id, authz.account_id, &security),
            path_params.thread_id,
        )
        .await?;

    Ok(StatusCode::OK)
}

fn delete_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete a thread")
        .description("Soft-deletes a thread and all of its comments. Requires ManageThreads.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Closes a thread, ending its discussion. Requires `ManageThreads`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        thread_id = %path_params.thread_id,
    )
)]
async fn close_thread(
    State(pg_client): State<PgClient>,
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::ManageThreads>,
    Path(path_params): Path<WorkspaceThreadPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<WorkspaceThread>)> {
    tracing::debug!(target: TRACING_TARGET, "Closing thread");

    let workspace = authz.workspace;
    let closed = threads
        .close(
            origin(workspace.id, authz.account_id, &security),
            path_params.thread_id,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = thread_response(&mut conn, closed).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn close_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Close a thread")
        .description("Closes a thread, ending the discussion. Requires ManageThreads.")
        .response::<200, Json<WorkspaceThread>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Reopens a closed thread. Requires `ManageThreads`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        thread_id = %path_params.thread_id,
    )
)]
async fn reopen_thread(
    State(pg_client): State<PgClient>,
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::ManageThreads>,
    Path(path_params): Path<WorkspaceThreadPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<WorkspaceThread>)> {
    tracing::debug!(target: TRACING_TARGET, "Reopening thread");

    let workspace = authz.workspace;
    let reopened = threads
        .reopen(
            origin(workspace.id, authz.account_id, &security),
            path_params.thread_id,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = thread_response(&mut conn, reopened).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn reopen_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reopen a thread")
        .description("Reopens a closed thread. Requires ManageThreads.")
        .response::<200, Json<WorkspaceThread>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Renames a thread (sets or clears its title). Requires `ManageThreads`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        thread_id = %path_params.thread_id,
    )
)]
async fn rename_thread(
    State(pg_client): State<PgClient>,
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::ManageThreads>,
    Path(path_params): Path<WorkspaceThreadPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<RenameWorkspaceThread>,
) -> Result<(StatusCode, Json<WorkspaceThread>)> {
    tracing::debug!(target: TRACING_TARGET, "Renaming thread");

    let workspace = authz.workspace;
    let renamed = threads
        .rename(
            origin(workspace.id, authz.account_id, &security),
            path_params.thread_id,
            request.display_name,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = thread_response(&mut conn, renamed).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn rename_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Rename a thread")
        .description("Sets or clears a thread's title. Requires ManageThreads.")
        .response::<200, Json<WorkspaceThread>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a thread's full timeline: comments and lifecycle events interleaved,
/// oldest first. Requires `ViewReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        thread_id = %path_params.thread_id,
    )
)]
async fn list_thread_timeline(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewReviews>,
    Path(path_params): Path<WorkspaceThreadPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspaceTimelinePage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing thread timeline");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    find_thread(&mut conn, workspace.id, path_params.thread_id).await?;

    // The timeline reads oldest first, so it walks ascending.
    let pagination = pagination
        .into_cursor::<TimelineCursor>()
        .with_direction(Direction::Ascending);
    let after = pagination.after_key();
    let fetch = pagination.fetch_limit();

    // Fetch a bounded window from each stream (fetch = limit + 1, so a full window
    // from either stream can still signal that more rows exist after the merge).
    let comments = conn
        .list_thread_comments_after(workspace.id, path_params.thread_id, after, fetch)
        .await?;
    let events = conn
        .list_thread_events_after(path_params.thread_id, after, fetch)
        .await?;

    // Merge the two already-ordered windows into one ascending timeline by
    // (created_at, source, id) — the same total order the cursor encodes.
    let mut entries: Vec<WorkspaceThreadEntry> = Vec::with_capacity(comments.len() + events.len());
    entries.extend(comments.into_iter().map(|row| {
        WorkspaceThreadEntry::Comment(WorkspaceComment::from_model(row.item, row.account.into()))
    }));
    entries.extend(events.into_iter().map(|(event, actor)| {
        WorkspaceThreadEntry::Event(WorkspaceThreadEvent::from_model(
            event,
            actor.map(Into::into),
        ))
    }));
    entries.sort_by_key(WorkspaceThreadEntry::sort_key);

    // The merged window holds up to 2 * fetch rows; a page is the first `limit`,
    // with a next cursor when a further entry exists beyond them.
    let response = WorkspaceTimelinePage::from_cursor_page(
        CursorPage::new(
            entries,
            None,
            pagination.limit,
            WorkspaceThreadEntry::cursor,
        ),
        |entry| entry,
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_thread_timeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List a thread's timeline")
        .description(
            "Returns the thread's timeline — comments and lifecycle events (opened, \
             closed, reopened, renamed, and review transitions) interleaved, oldest \
             first, with cursor pagination.",
        )
        .response::<200, Json<WorkspaceTimelinePage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Verifies a document's review, moving it to `resolved`.
///
/// Verification is whole-document: one gesture marks the entire review pass done.
/// Requires `Review` (a reviewer signs off their own work).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        document_id = %path_params.document_id,
    )
)]
async fn verify_review(
    State(pg_client): State<PgClient>,
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<WorkspaceThread>)> {
    tracing::debug!(target: TRACING_TARGET, "Verifying document review");

    let workspace = authz.workspace;
    let verified = threads
        .verify_review(
            origin(workspace.id, authz.account_id, &security),
            path_params.document_id,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = thread_response(&mut conn, verified).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn verify_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Verify a document review")
        .description(
            "Verifies a document's review as a whole, moving it to `resolved`. Requires \
             Review.",
        )
        .response::<200, Json<WorkspaceThread>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Assigns or unassigns a document's review.
///
/// A `null` assignee clears the current one. An assignee must be a workspace
/// member. Requires `AssignReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        document_id = %path_params.document_id,
    )
)]
async fn assign_review(
    State(pg_client): State<PgClient>,
    State(threads): State<domain::WorkspaceThreadService>,
    authz: Authorized<markers::AssignReviews>,
    Path(path_params): Path<WorkspaceDocumentPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<AssignWorkspaceReview>,
) -> Result<(StatusCode, Json<WorkspaceThread>)> {
    tracing::debug!(target: TRACING_TARGET, "Assigning document review");

    let workspace = authz.workspace;
    let updated = threads
        .assign_review(
            origin(workspace.id, authz.account_id, &security),
            path_params.document_id,
            request.assignee,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let response = thread_response(&mut conn, updated).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn assign_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Assign a document review")
        .description(
            "Assigns a document's review to a workspace member, or clears the assignee \
             with a null `assignee`. Requires AssignReviews.",
        )
        .response::<200, Json<WorkspaceThread>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Builds the event origin shared by every thread event.
fn origin<'a>(
    workspace_id: Uuid,
    account_id: Uuid,
    security: &'a SecurityContext,
) -> event::EventOrigin<'a> {
    event::EventOrigin {
        workspace_id,
        account_id,
        security,
    }
}

/// Builds a full [`WorkspaceThread`] response for `thread`, resolving its author
/// and (for a document review) its assignee.
async fn thread_response(
    conn: &mut PgConn,
    thread: nvisy_postgres::model::WorkspaceThread,
) -> Result<WorkspaceThread> {
    let author = resolve_account_ref(conn, thread.author_account_id).await?;
    let assignee = resolve_account_ref_opt(conn, thread.assignee_account_id).await?;
    Ok(WorkspaceThread::from_model(thread, author, assignee))
}

/// Finds a live thread in the workspace or returns a 404.
async fn find_thread(
    conn: &mut PgConn,
    workspace_id: Uuid,
    thread_id: Uuid,
) -> Result<nvisy_postgres::model::WorkspaceThread> {
    conn.find_thread_in_workspace(workspace_id, thread_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_thread"))
}

/// Returns an [`ApiRouter`] with the thread lifecycle and timeline routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/documents/{documentId}/review/verify/",
            post_with(verify_review, verify_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/documents/{documentId}/review/assign/",
            put_with(assign_review, assign_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/threads/",
            post_with(open_workspace_thread, open_workspace_thread_docs)
                .get_with(list_threads, list_threads_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/threads/{threadId}/",
            patch_with(rename_thread, rename_thread_docs)
                .delete_with(delete_thread, delete_thread_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/threads/{threadId}/close/",
            post_with(close_thread, close_thread_docs)
                .delete_with(reopen_thread, reopen_thread_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/threads/{threadId}/timeline/",
            get_with(list_thread_timeline, list_thread_timeline_docs),
        )
        .with_path_items(|item| item.tag("Threads"))
}
