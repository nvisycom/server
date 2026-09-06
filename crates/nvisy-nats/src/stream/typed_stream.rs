//! Type-safe wrappers over a JetStream pull consumer's messages.
//!
//! [`TypedMessageStream`] and [`TypedBatchStream`] deserialize a consumer's
//! messages into `T`; [`TypedMessage`] pairs a decoded value with its ack handle.

use std::marker::PhantomData;
use std::time::Duration;

use async_nats::jetstream::consumer::{self, Consumer};
use async_nats::jetstream::{self, Message};
use futures::StreamExt;
use serde::de::DeserializeOwned;

use crate::{Error, Result, TRACING_TARGET_STREAM};

/// Type-safe message stream wrapper.
///
/// Holds a single long-lived pull-consumer message stream and polls it
/// repeatedly. The stream is opened lazily on the first [`next`](Self::next) and
/// then reused, so messages already fetched into its buffer are not dropped
/// between calls.
pub struct TypedMessageStream<T> {
    consumer: Consumer<consumer::pull::Config>,
    messages: Option<consumer::pull::Stream>,
    _marker: PhantomData<T>,
}

impl<T> TypedMessageStream<T> {
    /// Wrap a pull consumer as a typed message stream.
    pub(super) fn new(consumer: Consumer<consumer::pull::Config>) -> Self {
        Self {
            consumer,
            messages: None,
            _marker: PhantomData,
        }
    }
}

impl<T> TypedMessageStream<T>
where
    T: DeserializeOwned + Send + 'static,
{
    /// Fetch the next message from the stream with timeout.
    pub async fn next_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<TypedMessage<T>>> {
        let result = tokio::time::timeout(timeout, self.next()).await;
        match result {
            Ok(msg_result) => msg_result,
            Err(_) => Ok(None), // Timeout occurred
        }
    }

    /// Fetch the next message from the persistent message stream.
    pub async fn next(&mut self) -> Result<Option<TypedMessage<T>>> {
        // Open the message stream once and reuse it across calls; recreating it
        // each call would drop any already-fetched-but-unpolled messages.
        if self.messages.is_none() {
            let messages = self
                .consumer
                .messages()
                .await
                .map_err(|e| Error::operation("messages_stream", e.to_string()))?;
            self.messages = Some(messages);
        }
        let messages = self.messages.as_mut().expect("message stream initialized");

        // Skip over poison messages (payloads that cannot be deserialized) by
        // terminating them, so a single bad message does not stall the consumer.
        loop {
            match messages.next().await {
                Some(Ok(message)) => match serde_json::from_slice::<T>(&message.payload) {
                    Ok(payload) => {
                        tracing::debug!(
                            target: TRACING_TARGET_STREAM,
                            subject = %message.subject,
                            "Received typed message"
                        );
                        return Ok(Some(TypedMessage { payload, message }));
                    }
                    Err(err) => {
                        // A payload that cannot be deserialized will never succeed,
                        // so terminate it (drop permanently) rather than leaving it
                        // un-acked to be redelivered until the stream ages it out.
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
                        // Continue to the next message.
                    }
                },
                Some(Err(e)) => {
                    tracing::warn!(
                        target: TRACING_TARGET_STREAM,
                        error = %e,
                        "Error receiving message"
                    );
                    return Err(Error::operation("message_receive", e.to_string()));
                }
                None => return Ok(None),
            }
        }
    }
}

/// Type-safe batch message stream wrapper.
pub struct TypedBatchStream<T> {
    consumer: Consumer<consumer::pull::Config>,
    batch_size: usize,
    _marker: PhantomData<T>,
}

impl<T> TypedBatchStream<T> {
    /// Wrap a pull consumer as a typed batch stream.
    pub(super) fn new(consumer: Consumer<consumer::pull::Config>, batch_size: usize) -> Self {
        Self {
            consumer,
            batch_size,
            _marker: PhantomData,
        }
    }
}

impl<T> TypedBatchStream<T>
where
    T: DeserializeOwned,
{
    /// Fetch the next batch of messages with timeout.
    pub async fn next_batch_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<TypedMessage<T>>> {
        let result = tokio::time::timeout(timeout, self.next_batch()).await;
        match result {
            Ok(batch_result) => batch_result,
            Err(_) => Ok(Vec::new()), // Timeout occurred, return empty batch
        }
    }

    /// Fetch the next batch of messages with custom batch size.
    pub async fn next_batch_sized(&mut self, batch_size: usize) -> Result<Vec<TypedMessage<T>>> {
        let mut batch = Vec::with_capacity(batch_size);

        match self
            .consumer
            .fetch()
            .max_messages(batch_size)
            .messages()
            .await
        {
            Ok(mut messages) => {
                while let Some(msg_result) = messages.next().await {
                    match msg_result {
                        Ok(message) => match serde_json::from_slice::<T>(&message.payload) {
                            Ok(payload) => {
                                batch.push(TypedMessage { payload, message });
                            }
                            Err(e) => {
                                tracing::warn!(
                                    target: TRACING_TARGET_STREAM,
                                    error = %e,
                                    "Failed to deserialize message payload in custom batch"
                                );
                                // Continue processing other messages
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                target: TRACING_TARGET_STREAM,
                                error = %e,
                                "Error receiving message in custom batch"
                            );
                        }
                    }
                }

                tracing::debug!(
                    target: TRACING_TARGET_STREAM,
                    batch_size = batch.len(),
                    requested_size = batch_size,
                    "Received custom-sized batch of typed messages"
                );

                Ok(batch)
            }
            Err(e) => Err(Error::operation("custom_batch_fetch", e.to_string())),
        }
    }

    /// Fetch the next batch of messages.
    pub async fn next_batch(&mut self) -> Result<Vec<TypedMessage<T>>> {
        let mut batch = Vec::with_capacity(self.batch_size);

        match self
            .consumer
            .fetch()
            .max_messages(self.batch_size)
            .messages()
            .await
        {
            Ok(mut messages) => {
                while let Some(msg_result) = messages.next().await {
                    match msg_result {
                        Ok(message) => match serde_json::from_slice::<T>(&message.payload) {
                            Ok(payload) => {
                                batch.push(TypedMessage { payload, message });
                            }
                            Err(e) => {
                                tracing::warn!(
                                    target: TRACING_TARGET_STREAM,
                                    error = %e,
                                    "Failed to deserialize message payload"
                                );
                                // Continue processing other messages
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                target: TRACING_TARGET_STREAM,
                                error = %e,
                                "Error receiving message in batch"
                            );
                        }
                    }
                }

                tracing::debug!(
                    target: TRACING_TARGET_STREAM,
                    batch_size = batch.len(),
                    "Received batch of typed messages"
                );

                Ok(batch)
            }
            Err(e) => Err(Error::operation("batch_fetch", e.to_string())),
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
    /// Get the message subject.
    pub fn subject(&self) -> &str {
        &self.message.subject
    }

    /// Get the message metadata.
    pub fn info(&self) -> Result<jetstream::message::Info<'_>> {
        self.message
            .info()
            .map_err(|e| Error::operation("message_info", e.to_string()))
    }

    /// Acknowledge the message.
    pub async fn ack(&mut self) -> Result<()> {
        self.message
            .ack()
            .await
            .map_err(|e| Error::operation("message_ack", e.to_string()))
    }

    /// Negative acknowledge the message (trigger redelivery).
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
    pub fn sequence(&self) -> Result<u64> {
        self.info()
            .map(|info| info.stream_sequence)
            .map_err(|e| Error::operation("get_sequence", e.to_string()))
    }

    /// Check if this message is a redelivery.
    pub fn is_redelivery(&self) -> Result<bool> {
        self.info()
            .map(|info| info.delivered > 1)
            .map_err(|e| Error::operation("check_redelivery", e.to_string()))
    }

    /// Get the number of delivery attempts.
    pub fn delivery_count(&self) -> Result<usize> {
        self.info()
            .map(|info| info.delivered as usize)
            .map_err(|e| Error::operation("get_delivery_count", e.to_string()))
    }

    /// Acknowledge with explicit acknowledgment kind.
    pub async fn ack_with(&mut self, ack_kind: jetstream::AckKind) -> Result<()> {
        self.message
            .ack_with(ack_kind)
            .await
            .map_err(|e| Error::operation("message_ack_with", e.to_string()))
    }

    /// Double acknowledge (useful for at-least-once processing).
    pub async fn double_ack(&mut self) -> Result<()> {
        self.message
            .double_ack()
            .await
            .map_err(|e| Error::operation("message_double_ack", e.to_string()))
    }
}
