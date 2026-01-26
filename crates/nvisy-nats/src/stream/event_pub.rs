//! Generic event stream publisher.

use std::marker::PhantomData;

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};
use serde::Serialize;

use super::event_stream::EventStream;
use super::stream_pub::StreamPublisher;
use crate::Result;

/// Generic event publisher for delivering typed events to workers.
///
/// This publisher is generic over:
/// - `T`: The event/message type to publish
/// - `S`: The stream configuration (determines stream name, subject, etc.)
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct EventPublisher<T, S>
where
    T: Serialize + Send + Sync + 'static,
    S: EventStream,
{
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<T>,
    _stream: PhantomData<S>,
}

impl<T, S> EventPublisher<T, S>
where
    T: Serialize + Send + Sync + 'static,
    S: EventStream,
{
    /// Create a new event publisher for the stream type.
    pub(crate) async fn new(jetstream: &Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, S::NAME).await?;
        Ok(Self {
            publisher,
            _stream: PhantomData,
        })
    }

    /// Publish an event to the stream's configured subject.
    pub async fn publish(&self, event: &T) -> Result<()> {
        self.publisher.publish(S::SUBJECT, event).await
    }

    /// Publish an event with a sub-subject appended to the stream subject.
    ///
    /// Events are published to `{stream_subject}.{sub_subject}`.
    pub async fn publish_to(&self, sub_subject: &str, event: &T) -> Result<()> {
        let subject = format!("{}.{}", S::SUBJECT, sub_subject);
        self.publisher.publish(&subject, event).await
    }

    /// Publish multiple events to the stream's configured subject.
    pub async fn publish_batch(&self, events: &[T]) -> Result<()>
    where
        T: Clone,
    {
        self.publisher.publish_batch(S::SUBJECT, events).await
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
