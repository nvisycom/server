//! Assistant reply worker.
//!
//! Consumes [`AssistantJob`]s from the `AssistantStream` work-queue and, in the
//! background, answers a comment that addressed the assistant: it reads the
//! thread's conversation, runs the workspace's language model over it, and posts
//! the reply as a comment authored by the reserved assistant account. That
//! comment flows through the normal comment-created event, so the thread's
//! timeline and mention notifications need no special handling here.

use std::sync::Arc;
use std::time::Duration;

use nvisy_inference::{ChatTurn, InferenceClient, InferenceConfig};
use nvisy_postgres::model::{NewWorkspaceThreadComment, WorkspaceThread, WorkspaceThreadComment};
use nvisy_postgres::query::{
    EventOutboxRepository, WorkspaceProviderRepository, WorkspaceThreadCommentRepository,
    WorkspaceThreadRepository,
};
use nvisy_postgres::types::ProviderType;
use nvisy_postgres::{ASSISTANT_ACCOUNT_ID, AsyncConnection, PgConn};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::job::{AssistantJob, AssistantStream};
use crate::extract::SecurityContext;
use crate::response::{Error, ErrorKind, Result};
use crate::service::{
    EventOrigin, Infra, ProviderConfig, ThreadCommentCreated, Worker, WorkspaceEvent,
    event_outbox_row,
};

/// Tracing target for assistant worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::assistant";

/// The system prompt that frames the assistant. Conversation-only: it does not
/// yet receive the pinned document's contents (a later enhancement), so it must
/// not claim to have read the file.
const PREAMBLE: &str = "You are the assistant for a document redaction platform. \
     You are replying inside a comment thread where a user has mentioned you. \
     Help the user understand and operate their workspace: redaction policies, \
     detections, pipelines, and the discussion in this thread. You do not have \
     access to the contents of any document. Be concise and accurate.";

/// Fallback concurrency when the runtime cannot report available parallelism.
const DEFAULT_ASSISTANT_CONCURRENCY: usize = 4;

/// Upper bound on one inference call, so a hung provider cannot pin a worker
/// task indefinitely. A timeout is transient — the job is redelivered.
const INFERENCE_TIMEOUT: Duration = Duration::from_secs(120);

/// Background worker that answers assistant mentions off the request thread.
///
/// Cheaply cloneable (every field is `Arc`-backed); a clone is handed to each
/// spawned per-job task so jobs run concurrently against the shared services.
#[derive(Clone)]
pub struct AssistantWorker {
    infra: Infra,
    /// Bounds how many assistant turns run at once. Inference is I/O-bound on the
    /// model provider, but the bound keeps a burst of mentions from opening an
    /// unbounded number of concurrent provider requests.
    concurrency: Arc<Semaphore>,
}

impl Worker for AssistantWorker {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "assistant"
    }

    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting assistant worker");

        let result = self.run_inner(cancel).await;

        match &result {
            Ok(()) => tracing::info!(target: TRACING_TARGET, "Assistant worker stopped"),
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Assistant worker failed")
            }
        }

        result
    }
}

impl AssistantWorker {
    /// Creates a new `AssistantWorker`.
    pub fn new(infra: Infra) -> Self {
        let concurrency = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(DEFAULT_ASSISTANT_CONCURRENCY);
        Self {
            infra,
            concurrency: Arc::new(Semaphore::new(concurrency)),
        }
    }

    /// Consumes assistant jobs until cancelled.
    ///
    /// At-least-once: a job is acked once it reaches a terminal outcome (a reply
    /// posted, or nothing left to do), and nacked for redelivery on a transient
    /// error (a DB/pool blip). Redelivery is idempotent because a job whose reply
    /// already landed is detected and skipped before a second reply is posted.
    async fn run_inner(&self, cancel: CancellationToken) -> Result<()> {
        let subscriber = self
            .infra
            .nats
            .event_subscriber::<AssistantStream>()
            .await?;
        let mut stream = subscriber.subscribe().await?;

        // In-flight per-job tasks are owned here rather than detached, so shutdown
        // can wait for them to settle their message (ack/nack). `JoinSet` also reaps
        // finished tasks so the set does not grow unbounded.
        let mut tasks: JoinSet<()> = JoinSet::new();

        loop {
            // Acquire a permit before pulling the next job so no more than
            // `concurrency` turns are ever in flight; the pull, and thus the
            // stream's redelivery lease, does not advance while every slot is busy.
            let permit = tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(target: TRACING_TARGET, "Assistant worker shutdown requested");
                    break;
                }
                Some(_) = tasks.join_next() => continue,
                permit = self.concurrency.clone().acquire_owned() => match permit {
                    Ok(permit) => permit,
                    Err(_) => break,
                },
            };

            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(target: TRACING_TARGET, "Assistant worker shutdown requested");
                    break;
                }
                result = stream.next_with_timeout(Duration::from_secs(5)) => {
                    match result {
                        Ok(Some(mut message)) => {
                            let job = message.payload().clone();
                            let worker = self.clone();
                            tasks.spawn(async move {
                                let _permit = permit;
                                let outcome = worker.run_job(job).await;
                                let ack_result = match outcome {
                                    JobOutcome::Done => message.ack().await,
                                    JobOutcome::Retry => message.nack().await,
                                };
                                if let Err(err) = ack_result {
                                    tracing::error!(target: TRACING_TARGET, error = %err, ?outcome, "Failed to ack/nack assistant job");
                                }
                            });
                        }
                        Ok(None) => drop(permit),
                        Err(err) => {
                            drop(permit);
                            tracing::error!(target: TRACING_TARGET, error = %err, "Error receiving assistant job");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }

        // Shutdown: stop pulling and let in-flight turns finish so each settles its
        // message. The app-wide shutdown timeout bounds this; a turn still running
        // when it fires is aborted and its message redelivered, which the reply
        // dedup makes idempotent.
        if !tasks.is_empty() {
            tracing::info!(
                target: TRACING_TARGET,
                in_flight = tasks.len(),
                "Draining in-flight assistant jobs before stopping",
            );
            while tasks.join_next().await.is_some() {}
        }
        Ok(())
    }

    /// Runs one assistant job: reads the thread, runs the model, posts the reply.
    ///
    /// Returns [`JobOutcome::Retry`] only for transient errors (no connection, a
    /// failed load or persist), so the reply is eventually posted. Terminal
    /// conditions — a missing thread/comment, a reply already posted, or no model
    /// provider configured — return [`JobOutcome::Done`]: retrying would not help.
    #[tracing::instrument(skip_all, fields(thread_id = %job.thread_id, comment_id = %job.comment_id, workspace_id = %job.workspace_id))]
    async fn run_job(&self, job: AssistantJob) -> JobOutcome {
        match self.reply(&job).await {
            Ok(()) => JobOutcome::Done,
            Err(ReplyError::Transient(err)) => {
                tracing::error!(target: TRACING_TARGET, error = %err, "Assistant reply failed transiently; will retry");
                JobOutcome::Retry
            }
            Err(ReplyError::Terminal(reason)) => {
                tracing::warn!(target: TRACING_TARGET, %reason, "Assistant reply not posted; dropping job");
                JobOutcome::Done
            }
        }
    }

    /// Loads the conversation, runs the model, and posts the reply. Distinguishes
    /// transient failures (worth a redelivery) from terminal ones (drop the job).
    ///
    /// A pooled connection is held only for the two database phases (the load and
    /// the post), never across the model call in between: inference is unbounded
    /// I/O against the provider, so pinning a pool connection to it would starve
    /// the pool under a slow provider. The `chat` call itself carries a timeout.
    async fn reply(&self, job: &AssistantJob) -> std::result::Result<(), ReplyError> {
        // Load phase: read the thread, the conversation, and the model client on
        // one connection, then drop it before inference.
        let (thread, prompt, history, client) = {
            let mut conn = self
                .infra
                .postgres
                .get_connection()
                .await
                .map_err(ReplyError::transient)?;

            // The thread must still exist and be live.
            let thread = conn
                .find_thread_in_workspace(job.workspace_id, job.thread_id)
                .await
                .map_err(ReplyError::transient)?
                .ok_or_else(|| ReplyError::terminal("thread no longer exists"))?;

            // Read the conversation oldest-first.
            let comments = conn
                .list_thread_comments(job.workspace_id, job.thread_id)
                .await
                .map_err(ReplyError::transient)?;

            // Idempotency: if the assistant has already replied to the triggering
            // comment, this is a redelivery — do not post a second reply.
            if already_replied(&comments, job.comment_id) {
                return Err(ReplyError::terminal("assistant already replied"));
            }

            // Resolve the workspace's language-model client. Only a genuinely
            // missing provider is terminal (retrying will not conjure one); a
            // decryption or client-build failure is transient, so redeliver rather
            // than silently dropping the job. `resolve_client` reports the
            // missing-provider case as a `Conflict`; every other failure is
            // treated as transient.
            let client = self
                .resolve_client(&mut conn, job.workspace_id)
                .await
                .map_err(|err| {
                    if err.kind() == ErrorKind::Conflict {
                        ReplyError::terminal("no language model provider configured")
                    } else {
                        ReplyError::transient(err)
                    }
                })?;

            // Build the turn: prior comments become history, the triggering comment
            // is the prompt. Skip the triggering comment in the history so it is not
            // duplicated as both history and prompt.
            let mut history = Vec::with_capacity(comments.len() + 1);
            history.push(ChatTurn::system(PREAMBLE));
            let mut prompt = String::new();
            for row in &comments {
                if row.item.id == job.comment_id {
                    prompt = row.item.body.clone();
                    continue;
                }
                history.push(turn_for(&row.item));
            }
            if prompt.is_empty() {
                // The triggering comment vanished (deleted) between enqueue and now.
                return Err(ReplyError::terminal("triggering comment no longer exists"));
            }

            (thread, prompt, history, client)
            // `conn` is dropped here, back to the pool, before inference runs.
        };

        // Inference phase: no connection held. Failures — provider timeouts, rate
        // limits (429), and 5xx — are transient: nack so the message is
        // redelivered rather than acking and leaving the user with no reply after a
        // short provider outage. A hung provider is bounded by `INFERENCE_TIMEOUT`.
        let answer = tokio::time::timeout(INFERENCE_TIMEOUT, client.chat(&prompt, history))
            .await
            .map_err(|_| {
                ReplyError::transient(
                    ErrorKind::ServiceUnavailable.with_message("Inference timed out"),
                )
            })?
            .map_err(ReplyError::transient)?;
        let answer = answer.trim();
        if answer.is_empty() {
            return Err(ReplyError::terminal("model returned an empty reply"));
        }

        // Post phase: acquire a fresh connection for the write. The database's
        // partial unique index on the triggering comment is the airtight guard: if
        // a live reply already exists (a redelivered job that raced past the
        // `already_replied` pre-check), the insert is rejected and nothing is
        // posted.
        let mut conn = self
            .infra
            .postgres
            .get_connection()
            .await
            .map_err(ReplyError::transient)?;
        let posted = self
            .post_reply(&mut conn, &thread, job.comment_id, answer)
            .await
            .map_err(ReplyError::transient)?;
        if !posted {
            return Err(ReplyError::terminal("assistant already replied"));
        }
        Ok(())
    }

    /// Resolves the workspace's language-model client from its configured LLM
    /// provider. Errors if none is configured or the client cannot be built.
    async fn resolve_client(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
    ) -> Result<InferenceClient> {
        let provider = conn
            .find_provider_by_type(workspace_id, ProviderType::Llm)
            .await?
            .ok_or_else(|| {
                ErrorKind::Conflict
                    .with_message("This workspace has no language model provider configured")
                    .with_resource("provider")
            })?;

        let config: ProviderConfig = self
            .infra
            .crypto
            .decrypt_json(workspace_id, &provider.encrypted_data)?;

        let llm = match config {
            ProviderConfig::Inference(InferenceConfig::Llm(llm)) => llm,
        };

        llm.connect(None).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to build the language model client")
                .with_context(err.to_string())
        })
    }

    /// Posts the assistant's reply as a comment answering `trigger_comment_id`,
    /// recording its comment-created event in the same transaction so the row and
    /// its event commit together. Authored by the reserved assistant account.
    ///
    /// Returns `false` (posting nothing) when a live reply to `trigger_comment_id`
    /// already exists — the database's partial unique index rejects the second
    /// insert, making a redelivered job a no-op.
    async fn post_reply(
        &self,
        conn: &mut PgConn,
        thread: &WorkspaceThread,
        trigger_comment_id: Uuid,
        body: &str,
    ) -> Result<bool> {
        conn.transaction(async |conn| {
            let Some(comment) = conn
                .create_reply(NewWorkspaceThreadComment {
                    workspace_id: thread.workspace_id,
                    thread_id: thread.id,
                    author_account_id: ASSISTANT_ACCOUNT_ID,
                    parent_id: Some(trigger_comment_id),
                    body: body.to_owned(),
                })
                .await?
            else {
                // A reply to this comment already exists; do not post again.
                return Ok(false);
            };

            // The assistant is the author, so there are no mentions to notify and
            // no @assistant self-trigger (the enqueue path only fires for a human
            // author addressing the assistant).
            let event = WorkspaceEvent::ThreadCommentCreated(ThreadCommentCreated {
                comment_id: comment.id,
                thread_id: thread.id,
                file_id: thread.file_id,
                author_username: nvisy_postgres::ASSISTANT_HANDLE.parse().map_err(|_| {
                    ErrorKind::InternalServerError.with_message("Invalid assistant handle")
                })?,
                mentioned: Vec::new(),
            });
            let row = event_outbox_row(
                EventOrigin {
                    workspace_id: thread.workspace_id,
                    account_id: ASSISTANT_ACCOUNT_ID,
                    security: &SecurityContext::default(),
                },
                &event,
            )?;
            conn.insert_event_outbox(row).await?;
            Ok::<_, Error>(true)
        })
        .await
    }
}

/// Whether the assistant has already replied to `comment_id` (the triggering
/// message) in this thread — the redelivery-dedup pre-check.
///
/// A reply is the assistant-authored comment whose `parent_id` is the triggering
/// comment (`post_reply` sets exactly that), so match it directly rather than by
/// iteration order. The database's partial unique index on `parent_id` is the
/// airtight guard; this only avoids the wasted inference of an obvious redelivery.
fn already_replied(
    comments: &[nvisy_postgres::types::WithAccountRef<WorkspaceThreadComment>],
    comment_id: Uuid,
) -> bool {
    comments.iter().any(|row| {
        row.item.author_account_id == ASSISTANT_ACCOUNT_ID && row.item.parent_id == Some(comment_id)
    })
}

/// Maps one stored comment to a chat turn: the assistant's own messages are the
/// assistant role, everyone else's are the user role.
fn turn_for(comment: &WorkspaceThreadComment) -> ChatTurn {
    if comment.author_account_id == ASSISTANT_ACCOUNT_ID {
        ChatTurn::assistant(comment.body.clone())
    } else {
        ChatTurn::user(comment.body.clone())
    }
}

/// Whether a consumed assistant job should be acked (done) or nacked (retry).
#[derive(Debug, Clone, Copy)]
enum JobOutcome {
    /// Reached a terminal outcome or is safe to drop; ack the message.
    Done,
    /// Transient error; nack for redelivery.
    Retry,
}

/// Why an assistant reply did not complete: a transient error (redeliver) or a
/// terminal condition (drop the job).
enum ReplyError {
    /// A transient failure (no connection, failed load or persist) — redeliver.
    Transient(Error<'static>),
    /// A terminal condition — retrying would not help, so drop the job. Carries a
    /// short reason (borrowed for the fixed cases, owned for a formatted one).
    Terminal(std::borrow::Cow<'static, str>),
}

impl ReplyError {
    /// Wraps a transient underlying error (any error convertible into the server
    /// error, e.g. a `nvisy_postgres::Error` from a repository call).
    fn transient(err: impl Into<Error<'static>>) -> Self {
        ReplyError::Transient(err.into())
    }

    /// A terminal condition with a reason (a static string or an owned message).
    fn terminal(reason: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        ReplyError::Terminal(reason.into())
    }
}
