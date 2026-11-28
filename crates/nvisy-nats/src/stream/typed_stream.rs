//! Type-safe wrappers over a `JetStream` pull consumer's messages.
//!
//! [`TypedMessageStream`] pulls a consumer's messages one at a time, each decoded
//! into `T` and paired with the raw message that settles it.

use std::marker::PhantomData;
use std::time::Duration;

use async_nats::jetstream::consumer::{self, Consumer};
use async_nats::jetstream::{self, Message};
use futures::StreamExt;
use serde::de::DeserializeOwned;
use tokio::time::Instant;

use crate::{Error, Result, TRACING_TARGET_STREAM};

/// The shortest pull worth sending; a shorter remaining wait ends the pull.
const MIN_PULL_WAIT: Duration = Duration::from_millis(50);

/// Extra time past a pull's expiry before the client stops waiting on a server
/// that has not answered.
const PULL_GRACE: Duration = Duration::from_secs(1);

/// Type-safe pull over a `JetStream` consumer.
///
/// Each [`next`](Self::next) issues a pull request for exactly one message, so
/// the server delivers a message only when the caller is ready to handle it.
/// Nothing is prefetched into a client-side buffer, where its ack lease would
/// run (and its delivery count go stale) while it waited for a handler.
pub struct TypedMessageStream<T> {
    consumer: Consumer<consumer::pull::Config>,
    _marker: PhantomData<T>,
}

impl<T> TypedMessageStream<T> {
    /// Wrap a pull consumer as a typed message stream.
    pub(super) fn new(consumer: Consumer<consumer::pull::Config>) -> Self {
        Self {
            consumer,
            _marker: PhantomData,
        }
    }
}

impl<T> TypedMessageStream<T>
where
    T: DeserializeOwned + Send + 'static,
{
    /// Pulls the next message, waiting up to `wait` for one to arrive, and
    /// returns its decoded payload with the raw message that settles it.
    ///
    /// The wait is the pull request's server-side expiry, so a request the
    /// server still holds never outlives the caller's interest in it. Every step
    /// (sending the request, receiving, terminating a poison message) is bounded
    /// by one absolute deadline, `wait` plus a short grace. A payload
    /// that cannot be deserialized is terminated (so the server stops
    /// redelivering it) and the pull continues within the same wait; a failure
    /// to terminate it is only logged.
    ///
    /// # Errors
    ///
    /// - `Operation` if the pull request cannot be sent or receiving the message
    ///   fails. No message within `wait` is not an error; it returns `Ok(None)`.
    pub async fn next(&self, wait: Duration) -> Result<Option<(T, Message)>> {
        let deadline = Instant::now() + wait;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining < MIN_PULL_WAIT {
                return Ok(None);
            }

            // Sending the request is bounded by the same deadline, so a stalled
            // connection cannot hold the caller past it.
            let request = self
                .consumer
                .batch()
                .max_messages(1)
                .expires(remaining)
                .messages();
            let mut batch = match tokio::time::timeout_at(deadline, request).await {
                Ok(batch) => batch.map_err(|e| Error::operation("pull_request", e.to_string()))?,
                Err(_) => return Ok(None),
            };
            // The server ends the batch when the request expires. The inbox stays
            // open for a grace past the expiry, so a delivery already in transit
            // is received rather than abandoned; the bound only guards against a
            // server that never answers.
            let pulled = tokio::time::timeout_at(deadline + PULL_GRACE, batch.next())
                .await
                .ok()
                .flatten();

            let message = match pulled {
                Some(Ok(message)) => message,
                Some(Err(e)) => {
                    tracing::warn!(target: TRACING_TARGET_STREAM, error = %e, "Error receiving message");
                    return Err(Error::operation("message_receive", e.to_string()));
                }
                None => return Ok(None),
            };

            match serde_json::from_slice::<T>(&message.payload) {
                Ok(payload) => {
                    tracing::debug!(
                        target: TRACING_TARGET_STREAM,
                        subject = %message.subject,
                        "Received typed message"
                    );
                    return Ok(Some((payload, message)));
                }
                Err(err) => {
                    // A payload that cannot be deserialized will never succeed, so
                    // terminate it rather than leave it to be redelivered until the
                    // stream ages it out.
                    tracing::error!(
                        target: TRACING_TARGET_STREAM,
                        subject = %message.subject,
                        error = %err,
                        "Terminating undeserializable message"
                    );
                    let term = message.ack_with(jetstream::AckKind::Term);
                    match tokio::time::timeout_at(deadline + PULL_GRACE, term).await {
                        Ok(Ok(())) => {}
                        Ok(Err(ack_err)) => tracing::warn!(
                            target: TRACING_TARGET_STREAM,
                            error = %ack_err,
                            "Failed to terminate undeserializable message"
                        ),
                        Err(_) => tracing::warn!(
                            target: TRACING_TARGET_STREAM,
                            "Timed out terminating undeserializable message"
                        ),
                    }
                }
            }
        }
    }
}
