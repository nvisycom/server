//! Type-safe wrappers over a `JetStream` pull consumer's messages.
//!
//! [`TypedMessageStream`] pulls a consumer's messages one at a time, decoded into
//! `T`;
//! [`TypedMessage`] pairs a decoded value with its ack handle.

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
    /// Pulls the next message, waiting up to `wait` for one to arrive.
    ///
    /// The wait is the pull request's server-side expiry, so a request the
    /// server still holds never outlives the caller's interest in it. A payload
    /// that cannot be deserialized is terminated (so the server stops
    /// redelivering it) and the pull continues within the same wait; a failure
    /// to terminate it is only logged.
    ///
    /// # Errors
    ///
    /// - `Operation` if the pull request cannot be sent or receiving the message
    ///   fails. No message within `wait` is not an error; it returns `Ok(None)`.
    pub async fn next(&self, wait: Duration) -> Result<Option<TypedMessage<T>>> {
        let deadline = Instant::now() + wait;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining < MIN_PULL_WAIT {
                return Ok(None);
            }

            let mut batch = self
                .consumer
                .batch()
                .max_messages(1)
                .expires(remaining)
                .messages()
                .await
                .map_err(|e| Error::operation("pull_request", e.to_string()))?;
            // The server ends the batch when the request expires; the client-side
            // bound only guards against a server that never answers.
            let pulled = tokio::time::timeout(remaining + PULL_GRACE, batch.next())
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
                    return Ok(Some(TypedMessage { payload, message }));
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
                    if let Err(ack_err) = message.ack_with(jetstream::AckKind::Term).await {
                        tracing::warn!(
                            target: TRACING_TARGET_STREAM,
                            error = %ack_err,
                            "Failed to terminate undeserializable message"
                        );
                    }
                }
            }
        }
    }
}

/// A typed message from the stream.
pub struct TypedMessage<T> {
    /// The deserialized payload.
    pub payload: T,
    /// The underlying NATS message for metadata and acknowledgment.
    message: Message,
}

impl<T> TypedMessage<T> {
    /// Splits the message into its payload and the raw message that settles it.
    pub(crate) fn into_parts(self) -> (T, Message) {
        (self.payload, self.message)
    }

    /// Get the message subject.
    pub fn subject(&self) -> &str {
        &self.message.subject
    }

    /// Get the message metadata.
    ///
    /// # Errors
    ///
    /// - `Operation` if the message's reply subject cannot be parsed into
    ///   `JetStream` metadata (e.g. it is not a `JetStream` message).
    pub fn info(&self) -> Result<jetstream::message::Info<'_>> {
        self.message
            .info()
            .map_err(|e| Error::operation("message_info", e.to_string()))
    }

    /// Acknowledge the message.
    ///
    /// # Errors
    ///
    /// - `Operation` if sending the ack to the server fails (e.g. the connection
    ///   is down).
    pub async fn ack(&mut self) -> Result<()> {
        self.message
            .ack()
            .await
            .map_err(|e| Error::operation("message_ack", e.to_string()))
    }

    /// Negative acknowledge the message (trigger redelivery).
    ///
    /// # Errors
    ///
    /// - `Operation` if sending the nack to the server fails (e.g. the connection
    ///   is down).
    pub async fn nack(&mut self) -> Result<()> {
        self.message
            .ack_with(jetstream::AckKind::Nak(None))
            .await
            .map_err(|e| Error::operation("message_nack", e.to_string()))
    }

    /// Get a reference to the typed payload.
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Consume the message and return the payload.
    pub fn into_payload(self) -> T {
        self.payload
    }

    /// Get message headers if available.
    pub fn headers(&self) -> Option<&async_nats::HeaderMap> {
        self.message.headers.as_ref()
    }

    /// Get message sequence number.
    ///
    /// # Errors
    ///
    /// - `Operation` if the message metadata cannot be read (see
    ///   [`info`](Self::info)).
    pub fn sequence(&self) -> Result<u64> {
        self.info()
            .map(|info| info.stream_sequence)
            .map_err(|e| Error::operation("get_sequence", e.to_string()))
    }

    /// Check if this message is a redelivery.
    ///
    /// # Errors
    ///
    /// - `Operation` if the message metadata cannot be read (see
    ///   [`info`](Self::info)).
    pub fn is_redelivery(&self) -> Result<bool> {
        self.info()
            .map(|info| info.delivered > 1)
            .map_err(|e| Error::operation("check_redelivery", e.to_string()))
    }

    /// Get the number of delivery attempts.
    ///
    /// # Errors
    ///
    /// - `Operation` if the message metadata cannot be read (see
    ///   [`info`](Self::info)).
    pub fn delivery_count(&self) -> Result<usize> {
        self.info()
            .map(|info| usize::try_from(info.delivered).unwrap_or(0))
            .map_err(|e| Error::operation("get_delivery_count", e.to_string()))
    }

    /// Acknowledge with explicit acknowledgment kind.
    ///
    /// # Errors
    ///
    /// - `Operation` if sending the acknowledgment to the server fails (e.g. the
    ///   connection is down).
    pub async fn ack_with(&mut self, ack_kind: jetstream::AckKind) -> Result<()> {
        self.message
            .ack_with(ack_kind)
            .await
            .map_err(|e| Error::operation("message_ack_with", e.to_string()))
    }

    /// Double acknowledge (useful for at-least-once processing).
    ///
    /// # Errors
    ///
    /// - `Operation` if the ack is sent but the server's confirmation is not
    ///   received (e.g. the connection is down).
    pub async fn double_ack(&mut self) -> Result<()> {
        self.message
            .double_ack()
            .await
            .map_err(|e| Error::operation("message_double_ack", e.to_string()))
    }
}
