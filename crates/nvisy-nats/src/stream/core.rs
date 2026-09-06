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

/// Ensures the stream backing `S` exists, creating it with `S`'s retention if
/// not. Idempotent: an existing stream is left as-is.
pub(super) async fn ensure_stream<S: EventStream>(jetstream: &Context) -> Result<()> {
    if jetstream.get_stream(S::NAME).await.is_ok() {
        tracing::trace!(target: TRACING_TARGET_STREAM, stream = %S::NAME, "Using existing stream");
        return Ok(());
    }

    let max_age = S::MAX_AGE.unwrap_or(DEFAULT_MAX_AGE);
    tracing::debug!(
        target: TRACING_TARGET_STREAM,
        stream = %S::NAME,
        max_age_secs = max_age.as_secs(),
        "Creating new stream"
    );
    jetstream
        .create_stream(stream::Config {
            name: S::NAME.to_string(),
            description: Some(format!("Type-safe stream: {}", S::NAME)),
            subjects: vec![format!("{}.>", S::NAME)],
            max_age,
            ..Default::default()
        })
        .await
        .map_err(|e| Error::operation("stream_create", e.to_string()))?;
    Ok(())
}
