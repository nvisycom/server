//! The [`EventStream`] contract and the shared stream-creation logic the typed
//! publisher and subscriber are built on.

use std::time::Duration;

use async_nats::jetstream::{Context, stream};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Error, Result, TRACING_TARGET_STREAM};

/// Marker trait for event streams.
///
/// Defines the configuration for a NATS JetStream stream, including the single
/// payload type it carries. A stream is a pure type-level tag — all of its
/// configuration lives in an associated type and consts — so it is never
/// instantiated and carries no value bounds.
pub trait EventStream: 'static {
    /// The payload type published to and consumed from this stream.
    type Message: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Stream name used in NATS JetStream.
    const NAME: &'static str;

    /// Human-readable description recorded on the stream, shown to operators
    /// (e.g. in `nats stream info`).
    const DESCRIPTION: &'static str;

    /// Subject pattern for publishing/subscribing to this stream.
    const SUBJECT: &'static str;

    /// Maximum age for messages in this stream.
    /// Returns `None` for streams where messages should not expire.
    const MAX_AGE: Option<Duration>;

    /// Default consumer name for this stream.
    const CONSUMER_NAME: &'static str;

    /// How long the server waits for an ack before redelivering a message.
    /// `None` uses the JetStream default (30s). Set this above the longest
    /// expected processing time so a slow-but-healthy job is not redelivered
    /// and run a second time concurrently.
    const ACK_WAIT: Option<Duration> = None;

    /// Maximum number of delivery attempts before the server stops redelivering a
    /// message. `None` means unlimited (redeliver until the message ages out).
    /// Set this to bound retries for consumers that nack on failure, so a
    /// permanently-failing message is not redelivered indefinitely.
    const MAX_DELIVER: Option<i64> = None;
}

/// Default stream retention when a stream has no `MAX_AGE`, shared by the
/// publisher and subscriber so both stream-creation paths agree regardless of
/// which runs first.
const DEFAULT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The subjects `S` binds: the exact [`SUBJECT`](EventStream::SUBJECT) that
/// [`publish`](super::EventPublisher::publish) uses, plus the `SUBJECT.>`
/// wildcard for the sub-subjects [`publish_to`](super::EventPublisher::publish_to)
/// appends. The stream and the consumer filter must both cover these, or a
/// published message the stream does not capture never gets a JetStream ack.
///
/// `SUBJECT.>` alone would not match a bare `publish` (the `>` requires a token
/// after the dot), so the exact subject is listed too.
pub(super) fn subjects<S: EventStream>() -> Vec<String> {
    vec![S::SUBJECT.to_string(), format!("{}.>", S::SUBJECT)]
}

/// Ensures the stream backing `S` exists and matches `S`'s current config.
///
/// Creates the stream if absent, and *reconciles* an existing one to `S`'s
/// subjects and retention. Reconciling matters because a stream is keyed by name
/// but its subjects can change across releases: a stream created by an earlier
/// version keeps its old subject filter, so publishing to the current
/// [`SUBJECT`](EventStream::SUBJECT) would match no stream and fail. Leaving the
/// old stream as-is (the previous behavior) let that drift break publishing
/// silently; updating it in place fixes it without an operator wiping JetStream.
pub(super) async fn ensure_stream<S: EventStream>(jetstream: &Context) -> Result<()> {
    let max_age = S::MAX_AGE.unwrap_or(DEFAULT_MAX_AGE);
    tracing::debug!(
        target: TRACING_TARGET_STREAM,
        stream = %S::NAME,
        max_age_secs = max_age.as_secs(),
        "Ensuring stream config",
    );
    jetstream
        .create_or_update_stream(stream::Config {
            name: S::NAME.to_string(),
            description: Some(S::DESCRIPTION.to_string()),
            subjects: subjects::<S>(),
            max_age,
            ..Default::default()
        })
        .await
        .map_err(|e| Error::operation("stream_ensure", e.to_string()))?;
    Ok(())
}
