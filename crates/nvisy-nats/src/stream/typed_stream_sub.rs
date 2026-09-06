//! Type-safe subscriber for JetStream streams.

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream::{Context, consumer, stream};
use serde::de::DeserializeOwned;

use super::typed_stream::{TypedBatchStream, TypedMessageStream};
use crate::{Error, Result, TRACING_TARGET_STREAM};

/// Inner data for TypedStreamSubscriber.
#[derive(Debug, Clone)]
struct TypedStreamSubscriberInner {
    jetstream: Context,
    stream_name: String,
    consumer_name: String,
    filter_subject: Option<String>,
    ack_wait: Option<Duration>,
    max_deliver: Option<i64>,
}

/// Type-safe stream subscriber with compile-time guarantees.
///
/// This subscriber provides a generic interface over JetStream for a specific
/// deserializable data type T, ensuring compile-time type safety for all receive
/// operations. The type parameter prevents mixing different message types.
#[derive(Debug, Clone)]
pub struct TypedStreamSubscriber<T> {
    inner: Arc<TypedStreamSubscriberInner>,
    _marker: PhantomData<T>,
}

impl<T> TypedStreamSubscriber<T>
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    /// Create a new type-safe stream subscriber.
    ///
    /// If the stream doesn't exist, it will be created with the specified max age
    /// (defaults to 24 hours if `None`).
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_STREAM)]
    pub(crate) async fn new_with_max_age(
        jetstream: &Context,
        stream_name: &str,
        consumer_name: &str,
        max_age: Option<Duration>,
        ack_wait: Option<Duration>,
        max_deliver: Option<i64>,
        filter_subject: Option<String>,
    ) -> Result<Self> {
        // Try to get existing stream, create if it doesn't exist
        match jetstream.get_stream(stream_name).await {
            Ok(_) => {
                tracing::trace!(
                    target: TRACING_TARGET_STREAM,
                    stream = %stream_name,
                    consumer = %consumer_name,
                    type_name = std::any::type_name::<T>(),
                    "Using existing stream for subscriber"
                );
            }
            Err(_) => {
                // Stream doesn't exist, create it
                let stream_config = stream::Config {
                    name: stream_name.to_string(),
                    description: Some(format!("Stream: {}", stream_name)),
                    subjects: vec![format!("{}.>", stream_name)],
                    max_age: max_age.unwrap_or(Duration::from_secs(24 * 60 * 60)), // Default 24 hours
                    ..Default::default()
                };

                tracing::debug!(
                    target: TRACING_TARGET_STREAM,
                    stream = %stream_name,
                    consumer = %consumer_name,
                    type_name = std::any::type_name::<T>(),
                    max_age_secs = ?max_age,
                    "Creating new stream for subscriber"
                );

                jetstream
                    .create_stream(stream_config)
                    .await
                    .map_err(|e| Error::operation("stream_create", e.to_string()))?;
            }
        }

        Ok(Self {
            inner: Arc::new(TypedStreamSubscriberInner {
                jetstream: jetstream.clone(),
                stream_name: stream_name.to_string(),
                consumer_name: consumer_name.to_string(),
                filter_subject,
                ack_wait,
                max_deliver,
            }),
            _marker: PhantomData,
        })
    }

    /// Subscribe to the stream and get a typed message stream.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn subscribe(&self) -> Result<TypedMessageStream<T>> {
        let mut consumer_config = consumer::pull::Config {
            durable_name: Some(self.inner.consumer_name.clone()),
            description: Some(format!("Consumer for stream {}", self.inner.stream_name)),
            ack_policy: consumer::AckPolicy::Explicit,
            ..Default::default()
        };

        if let Some(ack_wait) = self.inner.ack_wait {
            consumer_config.ack_wait = ack_wait;
        }

        if let Some(max_deliver) = self.inner.max_deliver {
            consumer_config.max_deliver = max_deliver;
        }

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

        tracing::debug!(
            target: TRACING_TARGET_STREAM,
            stream = %self.inner.stream_name,
            consumer = %self.inner.consumer_name,
            "Subscribed to stream"
        );

        Ok(TypedMessageStream::new(consumer))
    }

    /// Subscribe with a batch size for fetching messages.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_STREAM)]
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

        if let Some(ack_wait) = self.inner.ack_wait {
            consumer_config.ack_wait = ack_wait;
        }

        if let Some(max_deliver) = self.inner.max_deliver {
            consumer_config.max_deliver = max_deliver;
        }

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

        tracing::debug!(
            target: TRACING_TARGET_STREAM,
            stream = %self.inner.stream_name,
            consumer = %self.inner.consumer_name,
            batch_size = batch_size,
            "Subscribed to stream with batching"
        );

        Ok(TypedBatchStream::new(consumer, batch_size))
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
    #[tracing::instrument(skip(self), target = TRACING_TARGET_STREAM)]
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
                    tracing::debug!(
                        target: TRACING_TARGET_STREAM,
                        stream = %self.inner.stream_name,
                        consumer = %self.inner.consumer_name,
                        "Subscriber health check passed"
                    );
                    Ok(true)
                }
                Err(e) => {
                    tracing::debug!(
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
                tracing::debug!(
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
    #[tracing::instrument(skip(self), target = TRACING_TARGET_STREAM)]
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
