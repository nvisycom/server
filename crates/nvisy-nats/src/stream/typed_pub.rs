//! Type-safe JetStream publisher.
//!
//! A stream selects both its configuration and its payload type (see
//! [`EventStream`]), so [`EventPublisher`] is generic over `S` alone; the message
//! type is `S::Message`. It wraps the raw JetStream context directly, publishing
//! a serialized `S::Message` to the stream's subject.

use std::marker::PhantomData;

use async_nats::jetstream::Context;

use super::core::{EventStream, ensure_stream};
use crate::{Error, Result, TRACING_TARGET_STREAM};

/// Publishes a stream's typed events to its configured subject.
#[derive(Debug, Clone)]
pub struct EventPublisher<S: EventStream> {
    jetstream: Context,
    _stream: PhantomData<fn() -> S>,
}

impl<S: EventStream> EventPublisher<S> {
    /// Create a publisher for the stream, creating the stream if it is absent.
    pub(crate) async fn new(jetstream: &Context) -> Result<Self> {
        ensure_stream::<S>(jetstream).await?;
        Ok(Self {
            jetstream: jetstream.clone(),
            _stream: PhantomData,
        })
    }

    /// Publish an event to the stream's configured subject.
    pub async fn publish(&self, event: &S::Message) -> Result<()> {
        self.publish_subject(S::SUBJECT, event).await
    }

    /// Publish an event with a sub-subject appended to the stream subject.
    ///
    /// Events are published to `{stream_subject}.{sub_subject}`.
    pub async fn publish_to(&self, sub_subject: &str, event: &S::Message) -> Result<()> {
        let subject = format!("{}.{}", S::SUBJECT, sub_subject);
        self.publish_subject(&subject, event).await
    }

    /// Publish a serialized event to a fully-qualified subject.
    async fn publish_subject(&self, subject: &str, event: &S::Message) -> Result<()> {
        let payload = serde_json::to_vec(event).map_err(Error::Serialization)?;
        let payload_size = payload.len();

        self.jetstream
            .publish(subject.to_owned(), payload.into())
            .await
            .map_err(|e| Error::delivery_failed(subject, e.to_string()))?
            .await
            .map_err(|e| Error::operation("stream_publish", e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_STREAM,
            subject = %subject,
            payload_size = payload_size,
            "Published typed event"
        );
        Ok(())
    }

    /// Returns the stream name.
    #[inline]
    pub fn stream_name(&self) -> &'static str {
        S::NAME
    }

    /// Returns the subject.
    #[inline]
    pub fn subject(&self) -> &'static str {
        S::SUBJECT
    }
}
