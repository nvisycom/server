//! Type-safe subscriber for JetStream streams.

use std::marker::PhantomData;
use std::sync::Arc;

use async_nats::jetstream::consumer::{self, Consumer};
use async_nats::jetstream::{self, Context, Message};
use futures::StreamExt;
use serde::de::DeserializeOwned;
use tracing::{debug, instrument, warn};

use crate::{Error, Result, TRACING_TARGET_STREAM};

/// Inner data for StreamSubscriber.
#[derive(Debug, Clone)]
struct StreamSubscriberInner {
    jetstream: Context,
    stream_name: String,
    consumer_name: String,
    filter_subject: Option<String>,
}

/// Type-safe stream subscriber with compile-time guarantees.
///
/// This subscriber provides a generic interface over JetStream for a specific
/// deserializable data type T, ensuring compile-time type safety for all receive
/// operations. The type parameter prevents mixing different message types.
#[derive(Debug, Clone)]
pub struct StreamSubscriber<T> {
    inner: Arc<StreamSubscriberInner>,
    _marker: PhantomData<T>,
}

impl<T> StreamSubscriber<T>
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    /// Create a new type-safe stream subscriber.
    #[instrument(skip(jetstream), target = TRACING_TARGET_STREAM)]
    pub(crate) async fn new(
        jetstream: &Context,
        stream_name: &str,
        consumer_name: &str,
    ) -> Result<Self> {
        // Verify stream exists
        jetstream
            .get_stream(stream_name)
            .await
            .map_err(|e| Error::stream_error(stream_name, e.to_string()))?;

        debug!(
            target: TRACING_TARGET_STREAM,
            stream = %stream_name,
            consumer = %consumer_name,
            type_name = std::any::type_name::<T>(),
            "Created type-safe stream subscriber"
        );

        Ok(Self {
            inner: Arc::new(StreamSubscriberInner {
                jetstream: jetstream.clone(),
                stream_name: stream_name.to_string(),
                consumer_name: consumer_name.to_string(),
                filter_subject: None,
            }),
            _marker: PhantomData,
        })
    }

    /// Add a subject filter to the subscriber (builder pattern).
    pub fn with_filter_subject(self, filter: impl Into<String>) -> Self {
        let mut inner = Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone());
        inner.filter_subject = Some(filter.into());
        Self {
            inner: Arc::new(inner),
            _marker: PhantomData,
        }
    }

    /// Subscribe to the stream and get a typed message stream.
    #[instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn subscribe(&self) -> Result<TypedMessageStream<T>> {
        let mut consumer_config = consumer::pull::Config {
            durable_name: Some(self.inner.consumer_name.clone()),
            description: Some(format!("Consumer for stream {}", self.inner.stream_name)),
            ack_policy: consumer::AckPolicy::Explicit,
            ..Default::default()
        };

        if let Some(filter) = &self.inner.filter_subject {
            consumer_config.filter_subject = filter.clone();
        }

        // Get or create consumer
        let stream = self
            .inner
            .jetstream
            .get_stream(&self.inner.stream_name)
            .await
            .map_err(|e| {
                Error::stream_error(
                    &self.inner.stream_name,
                    format!("Failed to get stream: {}", e),
                )
            })?;

        let consumer = stream
            .get_or_create_consumer(&self.inner.consumer_name, consumer_config)
            .await
            .map_err(|e| {
                Error::consumer_error(
                    &self.inner.consumer_name,
                    format!("Failed to create consumer: {}", e),
                )
            })?;

        debug!(
            target: TRACING_TARGET_STREAM,
            stream = %self.inner.stream_name,
            consumer = %self.inner.consumer_name,
            "Subscribed to stream"
        );

        Ok(TypedMessageStream {
            consumer,
            _marker: PhantomData,
        })
    }

    /// Subscribe with a batch size for fetching messages.
    #[instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn subscribe_batch(&self, batch_size: usize) -> Result<TypedBatchStream<T>> {
        let mut consumer_config = consumer::pull::Config {
            durable_name: Some(self.inner.consumer_name.clone()),
            description: Some(format!(
                "Batch consumer for stream {}",
                self.inner.stream_name
            )),
            ack_policy: consumer::AckPolicy::Explicit,
            ..Default::default()
        };

        if let Some(filter) = &self.inner.filter_subject {
            consumer_config.filter_subject = filter.clone();
        }

        let stream = self
            .inner
            .jetstream
            .get_stream(&self.inner.stream_name)
            .await
            .map_err(|e| {
                Error::stream_error(
                    &self.inner.stream_name,
                    format!("Failed to get stream: {}", e),
                )
            })?;

        let consumer = stream
            .get_or_create_consumer(&self.inner.consumer_name, consumer_config)
            .await
            .map_err(|e| {
                Error::consumer_error(
                    &self.inner.consumer_name,
                    format!("Failed to create consumer: {}", e),
                )
            })?;

        debug!(
            target: TRACING_TARGET_STREAM,
            stream = %self.inner.stream_name,
            consumer = %self.inner.consumer_name,
            batch_size = batch_size,
            "Subscribed to stream with batching"
        );

        Ok(TypedBatchStream {
            consumer,
            batch_size,
            _marker: PhantomData,
        })
    }

    /// Get the stream name.
    #[inline]
    pub fn stream_name(&self) -> &str {
        &self.inner.stream_name
    }

    /// Get the consumer name.
    #[inline]
    pub fn consumer_name(&self) -> &str {
        &self.inner.consumer_name
    }

    /// Check if the stream and consumer are healthy and accessible.
    #[instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn health_check(&self) -> Result<bool> {
        match self
            .inner
            .jetstream
            .get_stream(&self.inner.stream_name)
            .await
        {
            Ok(stream) => match stream
                .get_consumer::<consumer::pull::Config>(&self.inner.consumer_name)
                .await
            {
                Ok(_) => {
                    debug!(
                        target: TRACING_TARGET_STREAM,
                        stream = %self.inner.stream_name,
                        consumer = %self.inner.consumer_name,
                        "Subscriber health check passed"
                    );
                    Ok(true)
                }
                Err(e) => {
                    debug!(
                        target: TRACING_TARGET_STREAM,
                        stream = %self.inner.stream_name,
                        consumer = %self.inner.consumer_name,
                        error = %e,
                        "Consumer health check failed"
                    );
                    Ok(false)
                }
            },
            Err(e) => {
                debug!(
                    target: TRACING_TARGET_STREAM,
                    stream = %self.inner.stream_name,
                    error = %e,
                    "Stream health check failed"
                );
                Ok(false)
            }
        }
    }

    /// Get consumer information.
    #[instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn consumer_info(&self) -> Result<consumer::Info> {
        let stream = self
            .inner
            .jetstream
            .get_stream(&self.inner.stream_name)
            .await
            .map_err(|e| Error::stream_error(&self.inner.stream_name, e.to_string()))?;

        let mut consumer = stream
            .get_consumer::<consumer::pull::Config>(&self.inner.consumer_name)
            .await
            .map_err(|e| Error::consumer_error(&self.inner.consumer_name, e.to_string()))?;

        consumer
            .info()
            .await
            .map_err(|e| Error::operation("consumer_info", e.to_string()))
            .map(|info| (*info).clone())
    }
}

/// Type-safe message stream wrapper.
pub struct TypedMessageStream<T> {
    consumer: Consumer<consumer::pull::Config>,
    _marker: PhantomData<T>,
}

impl<T> TypedMessageStream<T>
where
    T: DeserializeOwned + Send + 'static,
{
    /// Fetch the next message from the stream with timeout.
    pub async fn next_with_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Option<TypedMessage<T>>> {
        let result = tokio::time::timeout(timeout, self.next()).await;
        match result {
            Ok(msg_result) => msg_result,
            Err(_) => Ok(None), // Timeout occurred
        }
    }

    /// Fetch the next message from the stream.
    pub async fn next(&mut self) -> Result<Option<TypedMessage<T>>> {
        match self.consumer.messages().await {
            Ok(mut messages) => {
                if let Some(msg) = messages.next().await {
                    match msg {
                        Ok(message) => {
                            let payload: T = serde_json::from_slice(&message.payload)?;

                            debug!(
                                target: TRACING_TARGET_STREAM,
                                subject = %message.subject,
                                "Received typed message"
                            );

                            Ok(Some(TypedMessage { payload, message }))
                        }
                        Err(e) => {
                            warn!(
                                target: TRACING_TARGET_STREAM,
                                error = %e,
                                "Error receiving message"
                            );
                            Err(Error::operation("message_receive", e.to_string()))
                        }
                    }
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(Error::operation("messages_stream", e.to_string())),
        }
    }
}

/// Type-safe batch message stream wrapper.
pub struct TypedBatchStream<T> {
    consumer: Consumer<consumer::pull::Config>,
    batch_size: usize,
    _marker: PhantomData<T>,
}

impl<T> TypedBatchStream<T>
where
    T: DeserializeOwned,
{
    /// Fetch the next batch of messages with timeout.
    pub async fn next_batch_with_timeout(
        &mut self,
        timeout: std::time::Duration,
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
                                warn!(
                                    target: TRACING_TARGET_STREAM,
                                    error = %e,
                                    "Failed to deserialize message payload in custom batch"
                                );
                                // Continue processing other messages
                            }
                        },
                        Err(e) => {
                            warn!(
                                target: TRACING_TARGET_STREAM,
                                error = %e,
                                "Error receiving message in custom batch"
                            );
                        }
                    }
                }

                debug!(
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
                                warn!(
                                    target: TRACING_TARGET_STREAM,
                                    error = %e,
                                    "Failed to deserialize message payload"
                                );
                                // Continue processing other messages
                            }
                        },
                        Err(e) => {
                            warn!(
                                target: TRACING_TARGET_STREAM,
                                error = %e,
                                "Error receiving message in batch"
                            );
                        }
                    }
                }

                debug!(
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

#[cfg(test)]
mod tests {}
