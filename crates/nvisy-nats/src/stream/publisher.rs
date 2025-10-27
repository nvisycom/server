//! Type-safe publisher for JetStream streams.

use std::marker::PhantomData;
use std::sync::Arc;

use async_nats::jetstream::{self, stream};
use serde::Serialize;
use tokio::sync::Semaphore;
use tracing::{debug, instrument};

use crate::{Error, Result, TRACING_TARGET_STREAM};

/// Inner data for StreamPublisher
#[derive(Debug)]
struct StreamPublisherInner {
    jetstream: jetstream::Context,
    stream_name: String,
}

/// Type-safe stream publisher with compile-time guarantees
///
/// This publisher provides a generic interface over JetStream for a specific
/// serializable data type T, ensuring compile-time type safety for all publish
/// operations. The type parameter prevents mixing different message types.
#[derive(Debug, Clone)]
pub struct StreamPublisher<T> {
    inner: Arc<StreamPublisherInner>,
    _marker: PhantomData<T>,
}

impl<T> StreamPublisher<T>
where
    T: Serialize + Send + Sync + 'static,
{
    /// Create a new type-safe stream publisher
    #[instrument(skip(jetstream), target = TRACING_TARGET_STREAM)]
    pub async fn new(jetstream: &jetstream::Context, stream_name: &str) -> Result<Self> {
        let stream_config = stream::Config {
            name: stream_name.to_string(),
            description: Some(format!("Type-safe stream: {}", stream_name)),
            subjects: vec![format!("{}.>", stream_name)],
            max_age: std::time::Duration::from_secs(3600), // Keep messages for 1 hour
            ..Default::default()
        };

        // Try to get existing stream first
        match jetstream.get_stream(stream_name).await {
            Ok(_) => {
                debug!(
                    target: TRACING_TARGET_STREAM,
                    stream = %stream_name,
                    type_name = std::any::type_name::<T>(),
                    "Using existing stream"
                );
            }
            Err(_) => {
                // Stream doesn't exist, create it
                debug!(
                    target: TRACING_TARGET_STREAM,
                    stream = %stream_name,
                    type_name = std::any::type_name::<T>(),
                    max_age_secs = 3600,
                    "Creating new stream"
                );
                jetstream
                    .create_stream(stream_config)
                    .await
                    .map_err(|e| Error::operation("stream_create", e.to_string()))?;
            }
        }

        Ok(Self {
            inner: Arc::new(StreamPublisherInner {
                jetstream: jetstream.clone(),
                stream_name: stream_name.to_string(),
            }),
            _marker: PhantomData,
        })
    }

    /// Publish an event to the stream
    #[instrument(skip(self, event), target = TRACING_TARGET_STREAM)]
    pub async fn publish(&self, subject: &str, event: &T) -> Result<()> {
        let full_subject = format!("{}.{}", self.inner.stream_name, subject);
        let payload = serde_json::to_vec(event).map_err(Error::Serialization)?;
        let payload_size = payload.len();

        self.inner
            .jetstream
            .publish(full_subject.clone(), payload.into())
            .await
            .map_err(|e| Error::delivery_failed(&full_subject, e.to_string()))?
            .await
            .map_err(|e| Error::operation("stream_publish", e.to_string()))?;

        debug!(
            target: TRACING_TARGET_STREAM,
            subject = %full_subject,
            payload_size = payload_size,
            type_name = std::any::type_name::<T>(),
            "Published typed event"
        );
        Ok(())
    }

    /// Publish multiple events in batch with parallel processing
    #[instrument(skip(self, events), target = TRACING_TARGET_STREAM)]
    pub async fn publish_batch(&self, subject: &str, events: &[T]) -> Result<()>
    where
        T: Clone,
    {
        self.publish_batch_parallel(subject, events, 10).await
    }

    /// Publish multiple events in batch with configurable parallelism
    #[instrument(skip(self, events), target = TRACING_TARGET_STREAM)]
    pub async fn publish_batch_parallel(
        &self,
        subject: &str,
        events: &[T],
        parallelism: usize,
    ) -> Result<()>
    where
        T: Clone,
    {
        if events.is_empty() {
            return Ok(());
        }

        let count = events.len();
        let semaphore = Arc::new(Semaphore::new(parallelism));
        let mut tasks = Vec::with_capacity(events.len());

        for event in events.iter() {
            let event = event.clone();
            let subject = subject.to_string();
            let publisher = self.clone();
            let permit = semaphore.clone();

            let task = tokio::spawn(async move {
                let _permit = permit
                    .acquire()
                    .await
                    .map_err(|_| Error::operation("semaphore", "Failed to acquire permit"))?;
                publisher.publish(&subject, &event).await
            });

            tasks.push(task);
        }

        // Wait for all tasks and collect errors
        let mut errors = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(())) => {} // Success
                Ok(Err(e)) => errors.push(e),
                Err(e) => errors.push(Error::operation("task_join", e.to_string())),
            }
        }

        if !errors.is_empty() {
            return Err(Error::operation(
                "batch_publish",
                format!("Failed to publish {} out of {} events", errors.len(), count),
            ));
        }

        debug!(
            target: TRACING_TARGET_STREAM,
            count = count,
            parallelism = parallelism,
            stream = %self.inner.stream_name,
            subject = %subject,
            "Published batch of typed events in parallel"
        );
        Ok(())
    }

    /// Get the stream name
    pub fn stream_name(&self) -> &str {
        &self.inner.stream_name
    }

    /// Check if the stream is healthy and accessible
    #[instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn health_check(&self) -> Result<bool> {
        match self
            .inner
            .jetstream
            .get_stream(&self.inner.stream_name)
            .await
        {
            Ok(_) => {
                debug!(
                    target: TRACING_TARGET_STREAM,
                    stream = %self.inner.stream_name,
                    "Stream health check passed"
                );
                Ok(true)
            }
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

    /// Get stream information
    #[instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn stream_info(&self) -> Result<async_nats::jetstream::stream::Info> {
        let mut stream = self
            .inner
            .jetstream
            .get_stream(&self.inner.stream_name)
            .await
            .map_err(|e| Error::stream_error(&self.inner.stream_name, e.to_string()))?;

        stream
            .info()
            .await
            .map_err(|e| Error::operation("stream_info", e.to_string()))
            .map(|info| (*info).clone())
    }
}

#[cfg(test)]
mod tests {}
