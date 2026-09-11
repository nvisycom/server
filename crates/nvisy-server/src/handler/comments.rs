//! Comment handlers: threaded discussion on a file, with @-mentions and resolve.
//!
//! A comment is authored by a workspace member on a file, optionally a one-level
//! reply. `@username` mentions notify those workspace members. Viewing, writing,
//! and resolving all require the corresponding Reviewer-tier permission; editing
//! and deleting a comment are restricted to its author.

use std::collections::BTreeSet;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspaceComment, UpdateWorkspaceComment, WorkspaceComment};
use nvisy_postgres::query::{
    ReplyParentError, WorkspaceCommentRepository, WorkspaceFileRepository,
    WorkspaceMemberRepository,
};
use nvisy_postgres::types::Handle;
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CommentPathParams, CreateComment, CursorPagination, UpdateComment, WorkspaceCommentsQuery,
    WorkspaceFilePathParams,
};
use crate::handler::response::{Comment, CommentsPage};
use crate::handler::utility::resolve_account_ref;
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{
    CommentCreated, CommentDeleted, CommentResolved, EventEmitter, EventOrigin, ServiceState,
    WorkspaceEvent,
};

/// Tracing target for comment operations.
const TRACING_TARGET: &str = "nvisy_server::handler::comments";

/// Posts a comment on a file, or a reply to another comment.
///
/// A reply names its `parentId` (one level only). `@username` mentions in the
/// body notify those workspace members. Requires `Comment`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn create_comment(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::Comment>,
    Path(path_params): Path<WorkspaceFilePathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateComment>,
) -> Result<(StatusCode, Json<Comment>)> {
    tracing::debug!(target: TRACING_TARGET, "Posting comment");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // The file must exist in the workspace.
    conn.find_file_in_workspace(workspace.id, path_params.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;

    // Resolve @-mentions to workspace-member account ids, excluding the author
    // (no self-notification) and de-duplicated. A mentioned handle that is not a
    // workspace member is ignored rather than rejected.
    let mentioned =
        resolve_mentions(&mut conn, workspace.id, &request.body, authz.account_id).await?;

    // Store the typed anchor as its JSON; the DB column is modality-agnostic.
    let anchor = request
        .anchor
        .map(serde_json::to_value)
        .transpose()
        .map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to encode comment anchor")
                .with_context(err.to_string())
        })?;

    let new_comment = NewWorkspaceComment {
        workspace_id: workspace.id,
        file_id: path_params.file_id,
        author_account_id: authz.account_id,
        parent_id: request.parent_id,
        body: request.body,
        anchor,
    };

    let author_username = resolve_account_ref(&mut conn, authz.account_id)
        .await?
        .username;

    // Create the comment and record its event in one transaction so the row and
    // its event commit or roll back together.
    let comment = conn
        .transaction(async |conn| {
            let comment = if new_comment.parent_id.is_some() {
                match conn.create_reply(new_comment).await? {
                    Ok(comment) => comment,
                    Err(err) => return Ok(Err(err)),
                }
            } else {
                conn.create_comment(new_comment).await?
            };

            emit_comment_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                WorkspaceEvent::CommentCreated(CommentCreated {
                    comment_id: comment.id,
                    file_id: comment.file_id,
                    author_username: author_username.clone(),
                    mentioned,
                }),
            )
            .await?;
            Ok::<_, Error>(Ok(comment))
        })
        .await?
        .map_err(reply_parent_error)?;

    let author = resolve_account_ref(&mut conn, comment.author_account_id).await?;

    tracing::info!(target: TRACING_TARGET, comment_id = %comment.id, "Comment posted");

    Ok((
        StatusCode::CREATED,
        Json(Comment::from_model(comment, author)),
    ))
}

fn create_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Post a comment")
        .description(
            "Posts a comment on a file, or a reply to another comment (one level). \
             @username mentions notify those members. Requires the Comment permission.",
        )
        .response::<201, Json<Comment>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists a file's comments, oldest first (a thread reads top to bottom).
///
/// Requires `ViewComments`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn list_file_comments(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewComments>,
    Path(path_params): Path<WorkspaceFilePathParams>,
) -> Result<(StatusCode, Json<Vec<Comment>>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing file comments");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    conn.find_file_in_workspace(workspace.id, path_params.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;

    let rows = conn
        .list_file_comments(workspace.id, path_params.file_id)
        .await?;

    let comments = rows
        .into_iter()
        .map(|row| Comment::from_model(row.item, row.account.into()))
        .collect();

    Ok((StatusCode::OK, Json(comments)))
}

fn list_file_comments_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List a file's comments")
        .description("Returns the comments on a file, oldest first.")
        .response::<200, Json<Vec<Comment>>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists a workspace's comments with cursor pagination.
///
/// Filter by `fileId`, `author`, and `resolved`. Requires `ViewComments`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_workspace_comments(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewComments>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceCommentsQuery>,
) -> Result<(StatusCode, Json<CommentsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace comments");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_workspace_comments(workspace.id, pagination.into(), &query.into())
        .await?;

    let response = CommentsPage::from_cursor_page(page, |row| {
        Comment::from_model(row.item, row.account.into())
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_workspace_comments_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace comments")
        .description(
            "Returns the workspace's comments, most recent first, with optional \
             file, author, and resolved filters.",
        )
        .response::<200, Json<CommentsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Edits a comment's body. Restricted to the comment's author.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        comment_id = %path_params.comment_id,
    )
)]
async fn update_comment(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::Comment>,
    Path(path_params): Path<CommentPathParams>,
    ValidateJson(request): ValidateJson<UpdateComment>,
) -> Result<(StatusCode, Json<Comment>)> {
    tracing::debug!(target: TRACING_TARGET, "Editing comment");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let comment = find_comment(&mut conn, workspace.id, path_params.comment_id).await?;

    // Only the author may edit their own comment.
    if comment.author_account_id != authz.account_id {
        return Err(ErrorKind::Forbidden
            .with_message("Only the author can edit this comment")
            .with_resource("workspace_comment"));
    }

    let updated = conn
        .update_comment_body(
            comment.id,
            UpdateWorkspaceComment {
                body: Some(request.body),
            },
        )
        .await?;

    let author = resolve_account_ref(&mut conn, updated.author_account_id).await?;

    tracing::info!(target: TRACING_TARGET, "Comment edited");

    Ok((StatusCode::OK, Json(Comment::from_model(updated, author))))
}

fn update_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Edit a comment")
        .description("Edits a comment's body. Only the author may edit their own comment.")
        .response::<200, Json<Comment>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a comment (soft delete). Restricted to the comment's author.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        comment_id = %path_params.comment_id,
    )
)]
async fn delete_comment(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::Comment>,
    Path(path_params): Path<CommentPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting comment");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let comment = find_comment(&mut conn, workspace.id, path_params.comment_id).await?;

    // Only the author may delete their own comment.
    if comment.author_account_id != authz.account_id {
        return Err(ErrorKind::Forbidden
            .with_message("Only the author can delete this comment")
            .with_resource("workspace_comment"));
    }

    conn.transaction(async |conn| {
        conn.delete_comment(comment.id).await?;
        emit_comment_event(
            conn,
            workspace_origin(workspace.id, authz.account_id, &security),
            WorkspaceEvent::CommentDeleted(CommentDeleted {
                comment_id: comment.id,
                file_id: comment.file_id,
            }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

    tracing::info!(target: TRACING_TARGET, "Comment deleted");

    Ok(StatusCode::OK)
}

fn delete_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete a comment")
        .description("Soft-deletes a comment. Only the author may delete their own comment.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Resolves a comment thread. Requires `ResolveComments`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        comment_id = %path_params.comment_id,
    )
)]
async fn resolve_comment(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ResolveComments>,
    Path(path_params): Path<CommentPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<Comment>)> {
    tracing::debug!(target: TRACING_TARGET, "Resolving comment");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let comment = find_comment(&mut conn, workspace.id, path_params.comment_id).await?;
    require_top_level(&comment)?;

    // Already resolved: return it unchanged rather than overwriting the original
    // resolver/timestamp (the audit record) and emitting a duplicate event.
    if comment.resolved_at.is_some() {
        let author = resolve_account_ref(&mut conn, comment.author_account_id).await?;
        return Ok((StatusCode::OK, Json(Comment::from_model(comment, author))));
    }

    let resolved = conn
        .transaction(async |conn| {
            let resolved = conn.resolve_comment(comment.id, authz.account_id).await?;
            emit_comment_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                WorkspaceEvent::CommentResolved(CommentResolved {
                    comment_id: comment.id,
                    file_id: comment.file_id,
                }),
            )
            .await?;
            Ok::<_, Error>(resolved)
        })
        .await?;

    let author = resolve_account_ref(&mut conn, resolved.author_account_id).await?;

    tracing::info!(target: TRACING_TARGET, "Comment resolved");

    Ok((StatusCode::OK, Json(Comment::from_model(resolved, author))))
}

fn resolve_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Resolve a comment")
        .description("Resolves a comment thread, closing the discussion. Requires ResolveComments.")
        .response::<200, Json<Comment>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Reopens a resolved comment thread. Requires `ResolveComments`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        comment_id = %path_params.comment_id,
    )
)]
async fn reopen_comment(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ResolveComments>,
    Path(path_params): Path<CommentPathParams>,
) -> Result<(StatusCode, Json<Comment>)> {
    tracing::debug!(target: TRACING_TARGET, "Reopening comment");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let comment = find_comment(&mut conn, workspace.id, path_params.comment_id).await?;
    require_top_level(&comment)?;
    let reopened = conn.reopen_comment(comment.id).await?;
    let author = resolve_account_ref(&mut conn, reopened.author_account_id).await?;

    tracing::info!(target: TRACING_TARGET, "Comment reopened");

    Ok((StatusCode::OK, Json(Comment::from_model(reopened, author))))
}

fn reopen_comment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reopen a comment")
        .description("Reopens a resolved comment thread. Requires ResolveComments.")
        .response::<200, Json<Comment>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a live comment in the workspace or returns a 404.
async fn find_comment(
    conn: &mut PgConn,
    workspace_id: Uuid,
    comment_id: Uuid,
) -> Result<WorkspaceComment> {
    conn.find_comment_in_workspace(workspace_id, comment_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_comment"))
}

/// Rejects a reply where a top-level comment is required: a reply inherits its
/// thread's resolution state, so only the thread's top-level comment can be
/// resolved or reopened.
fn require_top_level(comment: &WorkspaceComment) -> Result<()> {
    if comment.parent_id.is_some() {
        return Err(ErrorKind::BadRequest
            .with_message("Resolve the thread's top-level comment, not a reply")
            .with_resource("workspace_comment"));
    }
    Ok(())
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

/// Parses `@username` mentions from `body`, resolving each to a workspace-member
/// account id — de-duplicated, excluding `author` (no self-notification), and
/// skipping handles that are not members of the workspace.
async fn resolve_mentions(
    conn: &mut PgConn,
    workspace_id: Uuid,
    body: &str,
    author: Uuid,
) -> Result<Vec<Uuid>> {
    // De-duplicate the raw mention text first (a repeated mention resolves once),
    // then parse each into a valid handle.
    let handles: Vec<Handle> = parse_mentions(body)
        .into_iter()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .filter_map(|m| Handle::parse(m).ok())
        .collect();

    if handles.is_empty() {
        return Ok(Vec::new());
    }

    // Resolve all mentioned handles to workspace-member account ids in one query,
    // then drop the author (no self-notification).
    let mut recipients = conn
        .find_member_ids_by_usernames(workspace_id, &handles)
        .await?;
    recipients.retain(|&id| id != author);
    Ok(recipients)
}

/// Maps a reply-parent validation failure to a client error.
fn reply_parent_error(err: ReplyParentError) -> Error<'static> {
    match err {
        ReplyParentError::NotFound => ErrorKind::NotFound.with_resource("workspace_comment"),
        ReplyParentError::FileMismatch => ErrorKind::BadRequest
            .with_message("The parent comment is on a different file")
            .with_resource("workspace_comment"),
        ReplyParentError::NotTopLevel => ErrorKind::BadRequest
            .with_message("Cannot reply to a reply; comment threads are one level deep")
            .with_resource("workspace_comment"),
    }
}

/// Builds the event origin shared by every comment event.
fn workspace_origin<'a>(
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

/// Emits one comment event onto the outbox.
async fn emit_comment_event(
    conn: &mut PgConn,
    origin: EventOrigin<'_>,
    event: WorkspaceEvent,
) -> Result<()> {
    conn.emit_event(origin, event).await?;
    Ok(())
}

/// Returns an [`ApiRouter`] with all comment routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/files/{fileId}/comments/",
            post_with(create_comment, create_comment_docs)
                .get_with(list_file_comments, list_file_comments_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/comments/",
            get_with(list_workspace_comments, list_workspace_comments_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/comments/{commentId}/",
            patch_with(update_comment, update_comment_docs)
                .delete_with(delete_comment, delete_comment_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/comments/{commentId}/resolve/",
            post_with(resolve_comment, resolve_comment_docs)
                .delete_with(reopen_comment, reopen_comment_docs),
        )
        .with_path_items(|item| item.tag("Comments"))
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
