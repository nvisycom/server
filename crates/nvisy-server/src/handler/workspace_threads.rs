//! Thread handlers: the thread lifecycle and its GitHub-issue-style timeline.
//! The messages within a thread are handled by the sibling
//! `workspace_thread_comments` module, which draws on the mention-resolution,
//! assistant-enqueue, and lookup helpers exported here.
//!
//! A thread is one of two things. A *workspace thread* is free-form discussion,
//! opened by a member with a first message and closable, reopenable, renamable,
//! or deletable as a whole. A *file thread* is a file's review: exactly one live
//! thread per file, auto-created on the file's first detection (never opened by
//! hand), carrying an optional assignee and a `review_status` derived from the
//! review timeline (detection → needs review, redaction → in review, verify →
//! resolved). Lifecycle and review transitions are recorded as timeline events
//! interleaved with the messages. `@username` mentions notify those workspace
//! members. Viewing (`ViewReviews`) and participating — commenting and verifying
//! a review (`Review`) — are Reviewer-tier; closing/reopening a workspace thread
//! (`ManageThreads`) and assigning a review (`AssignReviews`) are Editor-tier.

use std::collections::{BTreeSet, HashMap};

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{
    NewWorkspaceAssistantJob, NewWorkspaceThread, WorkspaceThread, WorkspaceThreadComment,
};
use nvisy_postgres::query::{
    AssistantJobOutboxRepository, TimelineCursor, WorkspaceFileRepository,
    WorkspaceMemberRepository, WorkspaceThreadCommentRepository, WorkspaceThreadEventRepository,
    WorkspaceThreadRepository,
};
use nvisy_postgres::types::{CursorPage, Direction, Handle};
use nvisy_postgres::{ASSISTANT_ACCOUNT_ID, ASSISTANT_HANDLE, AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    AssignReview, CursorPagination, OpenThread, RenameThread, ThreadPathParams,
    WorkspaceFilePathParams, WorkspaceThreadsQuery,
};
use crate::handler::response::{
    Comment, Thread, ThreadEntry, ThreadEvent, ThreadsPage, TimelinePage,
};
use crate::handler::utility::{resolve_account_ref, resolve_account_ref_opt};
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{
    AssistantJob, AssistantQueue, EventEmitter, EventOrigin, ReviewAssigned, ReviewUnassigned,
    ReviewVerified, ServiceState, ThreadClosed, ThreadDeleted, ThreadOpened, ThreadRenamed,
    ThreadReopened, WorkspaceEvent,
};

/// Tracing target for comment operations.
pub(crate) const TRACING_TARGET: &str = "nvisy_server::handler::comments";

/// Opens a workspace-level discussion thread (not tied to any file), with its
/// first message.
///
/// File reviews are auto-created on detection, not opened by hand, so this is the
/// only open endpoint. `@username` mentions in the opening body notify those
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
    State(assistant): State<AssistantQueue>,
    authz: Authorized<markers::Review>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<OpenThread>,
) -> Result<(StatusCode, Json<Thread>)> {
    tracing::debug!(target: TRACING_TARGET, "Opening workspace thread");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    open_thread(
        &mut conn,
        workspace.id,
        authz.account_id,
        &security,
        &assistant,
        request,
    )
    .await
}

fn open_workspace_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Open a workspace thread")
        .description(
            "Opens a workspace-level discussion thread (not tied to any file) with \
             its first message. @username mentions notify those members. Requires \
             the Review permission.",
        )
        .response::<201, Json<Thread>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Opens a workspace discussion thread with its opening comment, records the
/// opened event, and — if the assistant was addressed — queues its reply job,
/// all in one transaction.
async fn open_thread(
    conn: &mut PgConn,
    workspace_id: Uuid,
    author_id: Uuid,
    security: &SecurityContext,
    assistant: &AssistantQueue,
    request: OpenThread,
) -> Result<(StatusCode, Json<Thread>)> {
    // Resolve @-mentions to workspace-member account ids (author excluded,
    // de-duplicated; a non-member handle is ignored) and note whether the
    // assistant was addressed.
    let MentionOutcome {
        recipients,
        addressed_assistant,
    } = resolve_mentions(conn, workspace_id, &request.body, author_id).await?;

    // A workspace thread carries no file and no review status.
    let new_thread = NewWorkspaceThread {
        workspace_id,
        file_id: None,
        author_account_id: author_id,
        display_name: request.display_name,
        review_status: None,
    };

    let author_username = resolve_account_ref(conn, author_id).await?.username;

    // Open the thread (with its opening comment), record its event, and — if the
    // assistant was addressed — queue the reply job, all in one transaction so
    // the rows, the event, and the job commit or roll back together.
    let (thread, queued_assistant) = conn
        .transaction(async |conn| {
            let (thread, opening) = conn.open_thread(new_thread, request.body).await?;

            emit_thread_event(
                conn,
                workspace_origin(workspace_id, author_id, security),
                WorkspaceEvent::ThreadOpened(ThreadOpened {
                    thread_id: thread.id,
                    opening_comment_id: opening.id,
                    file_id: thread.file_id,
                    author_username: author_username.clone(),
                    mentioned: recipients,
                }),
            )
            .await?;

            let queued = enqueue_assistant_if_addressed(
                conn,
                addressed_assistant,
                author_id,
                workspace_id,
                thread.id,
                opening.id,
            )
            .await?;
            Ok::<_, Error>((thread, queued))
        })
        .await?;

    if queued_assistant {
        assistant.wake_drainer();
    }

    let response = thread_response(conn, thread).await?;

    tracing::info!(target: TRACING_TARGET, thread_id = %response.id, "Thread opened");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Lists a workspace's threads with cursor pagination, most recent first.
///
/// Filter by `fileId`, `author`, and `closed`. Requires `ViewReviews`.
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
) -> Result<(StatusCode, Json<ThreadsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing threads");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_threads(workspace.id, pagination.into_cursor(), &query.into())
        .await?;

    // Resolve the page's distinct assignees (file reviews) in one pass, so each
    // thread response can carry its assignee reference without an N+1 lookup.
    let assignee_ids: BTreeSet<Uuid> = page
        .items
        .iter()
        .filter_map(|row| row.item.assignee_account_id)
        .collect();
    let mut assignees: HashMap<Uuid, _> = HashMap::with_capacity(assignee_ids.len());
    for id in assignee_ids {
        if let Some(account) = resolve_account_ref_opt(&mut conn, Some(id)).await? {
            assignees.insert(id, account);
        }
    }

    let threads = page
        .items
        .into_iter()
        .map(|row| {
            let assignee = row
                .item
                .assignee_account_id
                .and_then(|id| assignees.get(&id).cloned());
            Thread::from_model(row.item, row.account.into(), assignee)
        })
        .collect();

    let response = ThreadsPage {
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
             file, author, and closed filters.",
        )
        .response::<200, Json<ThreadsPage>>()
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
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ManageThreads>,
    Path(path_params): Path<ThreadPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting thread");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let thread = find_thread(&mut conn, workspace.id, path_params.thread_id).await?;

    conn.transaction(async |conn| {
        conn.delete_thread(thread.id).await?;
        emit_thread_event(
            conn,
            workspace_origin(workspace.id, authz.account_id, &security),
            WorkspaceEvent::ThreadDeleted(ThreadDeleted {
                thread_id: thread.id,
                file_id: thread.file_id,
            }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

    tracing::info!(target: TRACING_TARGET, "Thread deleted");

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
    authz: Authorized<markers::ManageThreads>,
    Path(path_params): Path<ThreadPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<Thread>)> {
    tracing::debug!(target: TRACING_TARGET, "Closing thread");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let thread = find_thread(&mut conn, workspace.id, path_params.thread_id).await?;

    // Already closed: return it unchanged rather than overwriting the original
    // closer/timestamp (the audit record) and emitting a duplicate event.
    if thread.closed_at.is_some() {
        let response = thread_response(&mut conn, thread).await?;
        return Ok((StatusCode::OK, Json(response)));
    }

    let closed = conn
        .transaction(async |conn| {
            let closed = conn.close_thread(thread.id, authz.account_id).await?;
            emit_thread_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                WorkspaceEvent::ThreadClosed(ThreadClosed {
                    thread_id: thread.id,
                    file_id: thread.file_id,
                }),
            )
            .await?;
            Ok::<_, Error>(closed)
        })
        .await?;

    let response = thread_response(&mut conn, closed).await?;

    tracing::info!(target: TRACING_TARGET, "Thread closed");

    Ok((StatusCode::OK, Json(response)))
}

fn close_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Close a thread")
        .description("Closes a thread, ending the discussion. Requires ManageThreads.")
        .response::<200, Json<Thread>>()
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
    authz: Authorized<markers::ManageThreads>,
    Path(path_params): Path<ThreadPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<Thread>)> {
    tracing::debug!(target: TRACING_TARGET, "Reopening thread");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let thread = find_thread(&mut conn, workspace.id, path_params.thread_id).await?;

    // Already open: return it unchanged rather than emitting a duplicate event.
    if thread.closed_at.is_none() {
        let response = thread_response(&mut conn, thread).await?;
        return Ok((StatusCode::OK, Json(response)));
    }

    let reopened = conn
        .transaction(async |conn| {
            let reopened = conn.reopen_thread(thread.id, authz.account_id).await?;
            emit_thread_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                WorkspaceEvent::ThreadReopened(ThreadReopened {
                    thread_id: thread.id,
                    file_id: thread.file_id,
                }),
            )
            .await?;
            Ok::<_, Error>(reopened)
        })
        .await?;

    let response = thread_response(&mut conn, reopened).await?;

    tracing::info!(target: TRACING_TARGET, "Thread reopened");

    Ok((StatusCode::OK, Json(response)))
}

fn reopen_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reopen a thread")
        .description("Reopens a closed thread. Requires ManageThreads.")
        .response::<200, Json<Thread>>()
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
    authz: Authorized<markers::ManageThreads>,
    Path(path_params): Path<ThreadPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<RenameThread>,
) -> Result<(StatusCode, Json<Thread>)> {
    tracing::debug!(target: TRACING_TARGET, "Renaming thread");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let thread = find_thread(&mut conn, workspace.id, path_params.thread_id).await?;

    // The field is `Option<Option<String>>`: an absent `displayName` (`None`)
    // leaves the title unchanged, while an explicit `null` (`Some(None)`) clears
    // it. Only an explicit value triggers the update and its timeline event.
    let Some(display_name) = request.display_name else {
        let response = thread_response(&mut conn, thread).await?;
        return Ok((StatusCode::OK, Json(response)));
    };

    let renamed = conn
        .transaction(async |conn| {
            let renamed = conn
                .rename_thread(thread.id, display_name, authz.account_id)
                .await?;
            emit_thread_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                WorkspaceEvent::ThreadRenamed(ThreadRenamed {
                    thread_id: thread.id,
                    file_id: thread.file_id,
                }),
            )
            .await?;
            Ok::<_, Error>(renamed)
        })
        .await?;

    let response = thread_response(&mut conn, renamed).await?;

    tracing::info!(target: TRACING_TARGET, "Thread renamed");

    Ok((StatusCode::OK, Json(response)))
}

fn rename_thread_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Rename a thread")
        .description("Sets or clears a thread's title. Requires ManageThreads.")
        .response::<200, Json<Thread>>()
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
    Path(path_params): Path<ThreadPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<TimelinePage>)> {
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
    let mut entries: Vec<ThreadEntry> = Vec::with_capacity(comments.len() + events.len());
    entries.extend(
        comments
            .into_iter()
            .map(|row| ThreadEntry::Comment(Comment::from_model(row.item, row.account.into()))),
    );
    entries.extend(events.into_iter().map(|(event, actor)| {
        ThreadEntry::Event(ThreadEvent::from_model(event, actor.map(Into::into)))
    }));
    entries.sort_by_key(ThreadEntry::sort_key);

    // The merged window holds up to 2 * fetch rows; a page is the first `limit`,
    // with a next cursor when a further entry exists beyond them.
    let response = TimelinePage::from_cursor_page(
        CursorPage::new(entries, None, pagination.limit, ThreadEntry::cursor),
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
        .response::<200, Json<TimelinePage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Verifies a file's review, moving it to `resolved`.
///
/// Verification is whole-file: one gesture marks the entire review pass done.
/// Records a `review.verified` timeline event and raises a `review.verified`
/// workspace event. Requires `Review` (a reviewer signs off their own work).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn verify_review(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::Review>,
    Path(path_params): Path<WorkspaceFilePathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<Thread>)> {
    tracing::debug!(target: TRACING_TARGET, "Verifying file review");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let file = conn
        .find_file_in_workspace(workspace.id, path_params.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;
    let thread = conn
        .find_file_thread(workspace.id, path_params.file_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_thread"))?;

    let verified = conn
        .transaction(async |conn| {
            let verified = conn.verify_review(thread.id, authz.account_id).await?;
            emit_thread_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                WorkspaceEvent::ReviewVerified(ReviewVerified {
                    thread_id: thread.id,
                    file_id: file.id,
                    file_name: file.display_name.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>(verified)
        })
        .await?;

    let response = thread_response(&mut conn, verified).await?;

    tracing::info!(target: TRACING_TARGET, "File review verified");

    Ok((StatusCode::OK, Json(response)))
}

fn verify_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Verify a file review")
        .description(
            "Verifies a file's review as a whole, moving it to `resolved`. Requires \
             AssignReviews.",
        )
        .response::<200, Json<Thread>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Assigns or unassigns a file's review.
///
/// A `null` assignee clears the current one. An assignee must be a workspace
/// member. Records a `review.assigned` or `review.unassigned` timeline event and
/// raises the matching workspace event (assigning notifies the reviewer unless
/// they assigned themselves). Requires `AssignReviews`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn assign_review(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::AssignReviews>,
    Path(path_params): Path<WorkspaceFilePathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<AssignReview>,
) -> Result<(StatusCode, Json<Thread>)> {
    tracing::debug!(target: TRACING_TARGET, "Assigning file review");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let file = conn
        .find_file_in_workspace(workspace.id, path_params.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;
    let thread = conn
        .find_file_thread(workspace.id, path_params.file_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_thread"))?;

    // An assignee (when set) must be a workspace member, resolved to its handle
    // for the event; clearing needs no lookup.
    let assignee_ref = resolve_account_ref_opt(&mut conn, request.assignee).await?;
    if request.assignee.is_some() && assignee_ref.is_none() {
        return Err(Error::not_found("account"));
    }

    let updated = conn
        .transaction(async |conn| {
            let updated = conn
                .assign_review(thread.id, request.assignee, authz.account_id)
                .await?;
            let event = match (request.assignee, &assignee_ref) {
                (Some(assignee), Some(assignee_ref)) => {
                    WorkspaceEvent::ReviewAssigned(ReviewAssigned {
                        thread_id: thread.id,
                        file_id: file.id,
                        file_name: file.display_name.clone(),
                        assignee_username: assignee_ref.username.clone(),
                        // The reviewer is notified unless they assigned themselves.
                        notify: (assignee != authz.account_id).then_some(assignee),
                    })
                }
                _ => WorkspaceEvent::ReviewUnassigned(ReviewUnassigned {
                    thread_id: thread.id,
                    file_id: file.id,
                    file_name: Some(file.display_name.clone()),
                }),
            };
            emit_thread_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                event,
            )
            .await?;
            Ok::<_, Error>(updated)
        })
        .await?;

    let response = thread_response(&mut conn, updated).await?;

    tracing::info!(target: TRACING_TARGET, "File review assignment updated");

    Ok((StatusCode::OK, Json(response)))
}

fn assign_review_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Assign a file review")
        .description(
            "Assigns a file's review to a workspace member, or clears the assignee \
             with a null `assignee`. Requires AssignReviews.",
        )
        .response::<200, Json<Thread>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Builds a full [`Thread`] response for `thread`, resolving its author and (for
/// a file review) its assignee.
async fn thread_response(conn: &mut PgConn, thread: WorkspaceThread) -> Result<Thread> {
    let author = resolve_account_ref(conn, thread.author_account_id).await?;
    let assignee = resolve_account_ref_opt(conn, thread.assignee_account_id).await?;
    Ok(Thread::from_model(thread, author, assignee))
}

/// Finds a live thread in the workspace or returns a 404.
pub(crate) async fn find_thread(
    conn: &mut PgConn,
    workspace_id: Uuid,
    thread_id: Uuid,
) -> Result<WorkspaceThread> {
    conn.find_thread_in_workspace(workspace_id, thread_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_thread"))
}

/// Finds a live comment in the workspace or returns a 404.
pub(crate) async fn find_comment(
    conn: &mut PgConn,
    workspace_id: Uuid,
    comment_id: Uuid,
) -> Result<WorkspaceThreadComment> {
    conn.find_comment_in_workspace(workspace_id, comment_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_thread_comment"))
}

/// Extracts the raw handle text of each `@username` mention in `body`.
///
/// A mention is an `@` that starts a token (preceded by start-of-string or a
/// non-alphanumeric, non-`@` char, so an email's `@` is not a mention) followed
/// by handle characters (`[a-z0-9-]`). Validation (length, dash rules) is left to
/// [`Handle::parse`]; this only slices candidate spans.
fn parse_mentions(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut mentions = Vec::new();
    let mut i = 0;
    while let Some(at) = body[i..].find('@') {
        let at = i + at;
        // The `@` must begin a token: preceded by nothing, or by a char that is
        // not part of a word and not another `@`. A preceding ASCII alphanumeric
        // (so `a@b.com` is an email, not a mention) or any non-ASCII byte >= 0x80
        // (a multibyte letter like `é`, so `café@bob` is not a mention) counts as
        // part of a word.
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

/// The outcome of resolving a comment body's `@`-mentions.
pub(crate) struct MentionOutcome {
    /// Workspace-member account ids to notify (de-duplicated, author excluded).
    pub(crate) recipients: Vec<Uuid>,
    /// Whether the body addressed the reserved assistant handle (`@assistant`),
    /// so an AI reply should be queued. The assistant is not a workspace member,
    /// so it never appears in `recipients` — it is a job trigger, not a
    /// notification target.
    pub(crate) addressed_assistant: bool,
}

/// Parses `@username` mentions from `body`. Resolves each human handle to a
/// workspace-member account id — de-duplicated, excluding `author` (no
/// self-notification), and skipping handles that are not members — and separately
/// reports whether the reserved assistant handle was addressed.
pub(crate) async fn resolve_mentions(
    conn: &mut PgConn,
    workspace_id: Uuid,
    body: &str,
    author: Uuid,
) -> Result<MentionOutcome> {
    // De-duplicate the raw mention text first (a repeated mention resolves once),
    // then parse each into a valid handle.
    let raw: BTreeSet<String> = parse_mentions(body).into_iter().collect();

    // The assistant's reserved handle is recognized directly: it is not a
    // workspace member, so member resolution would never surface it.
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

    // Resolve all mentioned handles to workspace-member account ids in one query,
    // then drop the author (no self-notification).
    let mut recipients = conn
        .find_member_ids_by_usernames(workspace_id, &handles)
        .await?;
    recipients.retain(|&id| id != author);
    Ok(MentionOutcome {
        recipients,
        addressed_assistant,
    })
}

/// Queues an assistant-reply job for a just-created comment when it addressed the
/// assistant and was written by a human (not the assistant itself, so its own
/// replies never re-trigger it). Runs inside the comment's transaction so the job
/// commits atomically with the comment; returns whether a job was inserted, so
/// the caller can wake the drainer after commit. A serialization failure of the
/// tiny job payload is treated as fatal to the transaction (it should never
/// happen).
pub(crate) async fn enqueue_assistant_if_addressed(
    conn: &mut PgConn,
    addressed_assistant: bool,
    author_id: Uuid,
    workspace_id: Uuid,
    thread_id: Uuid,
    comment_id: Uuid,
) -> Result<bool> {
    if !addressed_assistant || author_id == ASSISTANT_ACCOUNT_ID {
        return Ok(false);
    }

    let job = AssistantJob {
        workspace_id,
        thread_id,
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

/// Builds the event origin shared by every comment event.
pub(crate) fn workspace_origin<'a>(
    workspace_id: Uuid,
    account_id: Uuid,
    security: &'a SecurityContext,
) -> EventOrigin<'a> {
    EventOrigin {
        workspace_id,
        account_id,
        security,
    }
}

/// Emits one thread collaboration event (a lifecycle change, a review
/// transition, or a new comment) onto the outbox.
pub(crate) async fn emit_thread_event(
    conn: &mut PgConn,
    origin: EventOrigin<'_>,
    event: WorkspaceEvent,
) -> Result<()> {
    conn.emit_event(origin, event).await?;
    Ok(())
}

/// Returns an [`ApiRouter`] with the thread lifecycle and timeline routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/files/{fileId}/review/verify/",
            post_with(verify_review, verify_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/files/{fileId}/review/assignee/",
            put_with(assign_review, assign_review_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/threads/",
            post_with(open_workspace_thread, open_workspace_thread_docs)
                .get_with(list_threads, list_threads_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/threads/{threadId}/",
            patch_with(rename_thread, rename_thread_docs)
                .delete_with(delete_thread, delete_thread_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/threads/{threadId}/close/",
            post_with(close_thread, close_thread_docs)
                .delete_with(reopen_thread, reopen_thread_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/threads/{threadId}/timeline/",
            get_with(list_thread_timeline, list_thread_timeline_docs),
        )
        .with_path_items(|item| item.tag("Threads"))
}

#[cfg(test)]
mod tests {
    use super::parse_mentions;

    #[test]
    fn parses_mentions_and_ignores_emails() {
        // A leading mention, a mid-sentence mention, and an email whose @ is not a
        // mention.
        assert_eq!(
            parse_mentions("@alice please review, cc @bob-smith — not user@example.com"),
            vec!["alice".to_owned(), "bob-smith".to_owned()],
        );
    }

    #[test]
    fn no_mentions_yields_empty() {
        assert!(parse_mentions("just a plain comment, no pings").is_empty());
        assert!(parse_mentions("").is_empty());
        // A bare @ with no handle text produces nothing.
        assert!(parse_mentions("look @ this").is_empty());
    }

    #[test]
    fn mention_stops_at_non_handle_chars() {
        // The handle ends at whitespace/punctuation; trailing text is not included.
        assert_eq!(parse_mentions("hey @carol!"), vec!["carol".to_owned()]);
        assert_eq!(parse_mentions("(@dave)"), vec!["dave".to_owned()]);
    }

    #[test]
    fn non_ascii_letter_before_at_is_not_a_boundary() {
        // A multibyte letter (é) before `@` means the `@` is embedded in a word,
        // not a mention — like an email local part.
        assert!(parse_mentions("café@bob").is_empty());
        // But a real mention after an accented word (with a space) still parses.
        assert_eq!(parse_mentions("café @bob"), vec!["bob".to_owned()]);
    }
}
