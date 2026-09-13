//! Workspace thread and comment domain logic: the thread lifecycle
//! (open/close/reopen/rename/delete), a document review's transitions
//! (verify/assign), and the messages within a thread (post/edit/delete).
//!
//! Threads and comments share one aggregate — mention resolution, the assistant
//! enqueue, and the timeline events — so they live in one service. It holds the
//! assistant queue to wake the reply drainer when a comment addresses the
//! assistant.

use std::collections::BTreeSet;

use nvisy_postgres::model::{
    NewWorkspaceAssistantJob, NewWorkspaceThread, NewWorkspaceThreadComment,
    UpdateWorkspaceThreadComment, WorkspaceThread, WorkspaceThreadComment,
};
use nvisy_postgres::query::{
    AccountRepository, AssistantJobOutboxRepository, WorkspaceDocumentRepository,
    WorkspaceMemberRepository, WorkspaceThreadCommentRepository, WorkspaceThreadRepository,
};
use nvisy_postgres::types::Handle;
use nvisy_postgres::{ASSISTANT_ACCOUNT_ID, ASSISTANT_HANDLE, AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::input::OpenThreadInput;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{AssistantJob, AssistantQueue, event};

/// Tracing target for thread domain operations.
const TRACING_TARGET: &str = "nvisy_server::service::thread";

/// Manages workspace threads, document-review transitions, and comments.
///
/// Holds the Postgres client (acquiring its own connection per call) and the
/// assistant queue (to wake the reply drainer when a comment addresses the
/// assistant). Resolved per request from
/// [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct WorkspaceThreadService {
    postgres: PgClient,
    assistant: AssistantQueue,
}

impl WorkspaceThreadService {
    /// Creates a [`WorkspaceThreadService`] over the connection pool and the
    /// assistant queue.
    pub fn new(postgres: PgClient, assistant: AssistantQueue) -> Self {
        Self {
            postgres,
            assistant,
        }
    }

    /// Opens a workspace discussion thread with its opening comment, records the
    /// opened event, and — if the assistant was addressed — queues its reply job,
    /// all in one transaction. Wakes the drainer after commit when a job was
    /// queued.
    pub async fn open(
        &self,
        origin: event::EventOrigin<'_>,
        input: OpenThreadInput,
    ) -> Result<WorkspaceThread> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let author_id = origin.account_id;

        let mentions = resolve_mentions(&mut conn, workspace_id, &input.body, author_id).await?;
        let author_username = account_username(&mut conn, author_id).await?;

        let new_thread = NewWorkspaceThread {
            workspace_id,
            document_id: None,
            author_account_id: author_id,
            display_name: input.display_name,
            review_status: None,
        };

        let (thread, queued_assistant) = conn
            .transaction(async |conn| {
                let (thread, opening) = conn.open_thread(new_thread, input.body).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ThreadOpened(event::ThreadOpened {
                        thread_id: thread.id,
                        opening_comment_id: opening.id,
                        document_id: thread.document_id,
                        author_username: author_username.clone(),
                        mentioned: mentions.recipients,
                    }),
                )
                .await?;
                let queued = enqueue_assistant_if_addressed(
                    conn,
                    mentions.addressed_assistant,
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
            self.assistant.wake_drainer();
        }

        tracing::info!(target: TRACING_TARGET, thread_id = %thread.id, "Thread opened");
        Ok(thread)
    }

    /// Deletes a thread and all of its comments (soft delete), recording the
    /// event atomically.
    pub async fn delete(&self, origin: event::EventOrigin<'_>, thread_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let thread = find_thread(&mut conn, origin.workspace_id, thread_id).await?;

        conn.transaction(async |conn| {
            conn.delete_thread(thread.id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::ThreadDeleted(event::ThreadDeleted {
                    thread_id: thread.id,
                    document_id: thread.document_id,
                }),
            )
            .await?;
            Ok::<_, Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Thread deleted");
        Ok(())
    }

    /// Closes a thread. An already-closed thread is returned unchanged, writing
    /// nothing and emitting no duplicate event.
    pub async fn close(
        &self,
        origin: event::EventOrigin<'_>,
        thread_id: Uuid,
    ) -> Result<WorkspaceThread> {
        let mut conn = self.postgres.get_connection().await?;
        let thread = find_thread(&mut conn, origin.workspace_id, thread_id).await?;

        if thread.closed_at.is_some() {
            return Ok(thread);
        }

        let closed = conn
            .transaction(async |conn| {
                let closed = conn.close_thread(thread.id, origin.account_id).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ThreadClosed(event::ThreadClosed {
                        thread_id: thread.id,
                        document_id: thread.document_id,
                    }),
                )
                .await?;
                Ok::<_, Error>(closed)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Thread closed");
        Ok(closed)
    }

    /// Reopens a closed thread. An already-open thread is returned unchanged,
    /// emitting no duplicate event.
    pub async fn reopen(
        &self,
        origin: event::EventOrigin<'_>,
        thread_id: Uuid,
    ) -> Result<WorkspaceThread> {
        let mut conn = self.postgres.get_connection().await?;
        let thread = find_thread(&mut conn, origin.workspace_id, thread_id).await?;

        if thread.closed_at.is_none() {
            return Ok(thread);
        }

        let reopened = conn
            .transaction(async |conn| {
                let reopened = conn.reopen_thread(thread.id, origin.account_id).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ThreadReopened(event::ThreadReopened {
                        thread_id: thread.id,
                        document_id: thread.document_id,
                    }),
                )
                .await?;
                Ok::<_, Error>(reopened)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Thread reopened");
        Ok(reopened)
    }

    /// Renames a thread. `display_name` is `Option<Option<String>>`: `None` leaves
    /// the title unchanged (returned as-is), `Some(None)` clears it, `Some(Some)`
    /// sets it. Only an explicit value writes and emits a timeline event.
    pub async fn rename(
        &self,
        origin: event::EventOrigin<'_>,
        thread_id: Uuid,
        display_name: Option<Option<String>>,
    ) -> Result<WorkspaceThread> {
        let mut conn = self.postgres.get_connection().await?;
        let thread = find_thread(&mut conn, origin.workspace_id, thread_id).await?;

        let Some(display_name) = display_name else {
            return Ok(thread);
        };

        let renamed = conn
            .transaction(async |conn| {
                let renamed = conn
                    .rename_thread(thread.id, display_name, origin.account_id)
                    .await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ThreadRenamed(event::ThreadRenamed {
                        thread_id: thread.id,
                        document_id: thread.document_id,
                    }),
                )
                .await?;
                Ok::<_, Error>(renamed)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Thread renamed");
        Ok(renamed)
    }

    /// Verifies a document's review as a whole, moving it to `resolved`, and
    /// raises the review-verified event.
    pub async fn verify_review(
        &self,
        origin: event::EventOrigin<'_>,
        document_id: Uuid,
    ) -> Result<WorkspaceThread> {
        let mut conn = self.postgres.get_connection().await?;
        let document = conn
            .find_document_in_workspace(origin.workspace_id, document_id)
            .await?
            .ok_or_else(|| Error::not_found("document"))?;
        let thread = conn
            .find_document_thread(origin.workspace_id, document_id)
            .await?
            .ok_or_else(|| Error::not_found("workspace_thread"))?;

        let verified = conn
            .transaction(async |conn| {
                let verified = conn.verify_review(thread.id, origin.account_id).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ReviewVerified(event::ReviewVerified {
                        thread_id: thread.id,
                        document_id: document.id,
                        document_name: document.display_name.clone(),
                    }),
                )
                .await?;
                Ok::<_, Error>(verified)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Document review verified");
        Ok(verified)
    }

    /// Assigns or unassigns a document's review. A `null` assignee clears the
    /// current one; a set assignee must be a workspace member (else a NotFound),
    /// resolved to its handle for the event. Raises the matching review event,
    /// notifying the assignee unless they assigned themselves.
    pub async fn assign_review(
        &self,
        origin: event::EventOrigin<'_>,
        document_id: Uuid,
        assignee: Option<Uuid>,
    ) -> Result<WorkspaceThread> {
        let mut conn = self.postgres.get_connection().await?;
        let document = conn
            .find_document_in_workspace(origin.workspace_id, document_id)
            .await?
            .ok_or_else(|| Error::not_found("document"))?;
        let thread = conn
            .find_document_thread(origin.workspace_id, document_id)
            .await?
            .ok_or_else(|| Error::not_found("workspace_thread"))?;

        // A set assignee must be a workspace member; resolving through membership
        // also keeps a non-member's identity from being exposed across workspaces.
        let assignee_username = match assignee {
            Some(assignee) => Some(
                conn.find_workspace_member_with_account(origin.workspace_id, assignee)
                    .await?
                    .map(|(_, account)| account.username)
                    .ok_or_else(|| Error::not_found("account"))?,
            ),
            None => None,
        };

        let actor_id = origin.account_id;
        let updated = conn
            .transaction(async |conn| {
                let updated = conn.assign_review(thread.id, assignee, actor_id).await?;
                let event = match (assignee, assignee_username) {
                    (Some(assignee), Some(username)) => {
                        event::WorkspaceEvent::ReviewAssigned(event::ReviewAssigned {
                            thread_id: thread.id,
                            document_id: document.id,
                            document_name: document.display_name.clone(),
                            assignee_username: username,
                            // The reviewer is notified unless they assigned themselves.
                            notify: (assignee != actor_id).then_some(assignee),
                        })
                    }
                    _ => event::WorkspaceEvent::ReviewUnassigned(event::ReviewUnassigned {
                        thread_id: thread.id,
                        document_id: document.id,
                        document_name: Some(document.display_name.clone()),
                    }),
                };
                conn.emit_event(origin, event).await?;
                Ok::<_, Error>(updated)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Document review assignment updated");
        Ok(updated)
    }

    /// Posts a comment in a thread, records its event, and — if the assistant was
    /// addressed — queues the reply job, all in one transaction. Rejects a comment
    /// on a closed thread with a Conflict. Wakes the drainer after commit when a
    /// job was queued.
    pub async fn create_comment(
        &self,
        origin: event::EventOrigin<'_>,
        thread_id: Uuid,
        body: String,
    ) -> Result<WorkspaceThreadComment> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let author_id = origin.account_id;

        // The thread must exist in the workspace (and be live).
        let thread = find_thread(&mut conn, workspace_id, thread_id).await?;
        let mentions = resolve_mentions(&mut conn, workspace_id, &body, author_id).await?;
        let author_username = account_username(&mut conn, author_id).await?;

        let (comment, queued_assistant) = conn
            .transaction(async |conn| {
                // Lock the thread and re-check its closed state inside the
                // transaction: the row lock serializes against a concurrent close
                // so a comment (and its event) can never land after ThreadClosed.
                let locked = conn
                    .lock_thread_in_workspace(workspace_id, thread.id)
                    .await?
                    .ok_or_else(|| Error::not_found("workspace_thread"))?;
                if locked.closed_at.is_some() {
                    return Err(ErrorKind::Conflict.with_message(
                        "This thread is closed; reopen it before posting a comment",
                    ));
                }

                let comment = conn
                    .create_comment(NewWorkspaceThreadComment {
                        workspace_id,
                        thread_id: thread.id,
                        author_account_id: author_id,
                        parent_id: None,
                        body,
                    })
                    .await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ThreadCommentCreated(event::ThreadCommentCreated {
                        comment_id: comment.id,
                        thread_id: thread.id,
                        document_id: thread.document_id,
                        author_username: author_username.clone(),
                        mentioned: mentions.recipients,
                    }),
                )
                .await?;
                let queued = enqueue_assistant_if_addressed(
                    conn,
                    mentions.addressed_assistant,
                    author_id,
                    workspace_id,
                    thread.id,
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
    pub async fn update_comment(
        &self,
        workspace_id: Uuid,
        account_id: Uuid,
        comment_id: Uuid,
        body: String,
    ) -> Result<WorkspaceThreadComment> {
        let mut conn = self.postgres.get_connection().await?;
        let comment = find_comment(&mut conn, workspace_id, comment_id).await?;
        if comment.author_account_id != account_id {
            return Err(ErrorKind::Forbidden
                .with_message("Only the author can edit this comment")
                .with_resource("workspace_thread_comment"));
        }

        let updated = conn
            .update_comment_body(
                comment.id,
                UpdateWorkspaceThreadComment { body: Some(body) },
            )
            .await?;

        tracing::info!(target: TRACING_TARGET, "Comment edited");
        Ok(updated)
    }

    /// Soft-deletes a comment. Restricted to the comment's author.
    pub async fn delete_comment(
        &self,
        workspace_id: Uuid,
        account_id: Uuid,
        comment_id: Uuid,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let comment = find_comment(&mut conn, workspace_id, comment_id).await?;
        if comment.author_account_id != account_id {
            return Err(ErrorKind::Forbidden
                .with_message("Only the author can delete this comment")
                .with_resource("workspace_thread_comment"));
        }

        conn.delete_comment(comment.id).await?;
        tracing::info!(target: TRACING_TARGET, "Comment deleted");
        Ok(())
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

/// The username of an account by id, or a NotFound.
async fn account_username(conn: &mut PgConn, account_id: Uuid) -> Result<Handle> {
    conn.find_account_by_id(account_id)
        .await?
        .map(|account| account.username)
        .ok_or_else(|| Error::not_found("account"))
}

/// Finds a live thread in the workspace or returns a 404.
async fn find_thread(
    conn: &mut PgConn,
    workspace_id: Uuid,
    thread_id: Uuid,
) -> Result<WorkspaceThread> {
    conn.find_thread_in_workspace(workspace_id, thread_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_thread"))
}

/// Finds a live comment in the workspace or returns a 404.
async fn find_comment(
    conn: &mut PgConn,
    workspace_id: Uuid,
    comment_id: Uuid,
) -> Result<WorkspaceThreadComment> {
    conn.find_comment_in_workspace(workspace_id, comment_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_thread_comment"))
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
