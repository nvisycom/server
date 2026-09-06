//! Generic event stream publisher and subscriber.
//!
//! A stream selects both its configuration and its payload type (see
//! [`EventStream`]), so each is generic over `S` alone; the message type is
//! `S::Message`.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::event_stream::EventStream;
use super::typed_stream_pub::TypedStreamPublisher;
use super::typed_stream_sub::TypedStreamSubscriber;
use crate::Result;

/// Generic event publisher for delivering a stream's typed events to workers.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct EventPublisher<S: EventStream> {
    #[deref]
    #[deref_mut]
    publisher: TypedStreamPublisher<S::Message>,
}

impl<S: EventStream> EventPublisher<S> {
    /// Create a new event publisher for the stream type.
    pub(crate) async fn new(jetstream: &Context) -> Result<Self> {
        let publisher = TypedStreamPublisher::new(jetstream, S::NAME).await?;
        Ok(Self { publisher })
    }

    /// Publish an event to the stream's configured subject.
    pub async fn publish(&self, event: &S::Message) -> Result<()> {
        self.publisher.publish(S::SUBJECT, event).await
    }

    /// Publish an event with a sub-subject appended to the stream subject.
    ///
    /// Events are published to `{stream_subject}.{sub_subject}`.
    pub async fn publish_to(&self, sub_subject: &str, event: &S::Message) -> Result<()> {
        let subject = format!("{}.{}", S::SUBJECT, sub_subject);
        self.publisher.publish(&subject, event).await
    }

    /// Publish multiple events to the stream's configured subject.
    pub async fn publish_batch(&self, events: &[S::Message]) -> Result<()>
    where
        S::Message: Clone,
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

/// Generic event subscriber for consuming a stream's typed events.
#[derive(Debug, Deref, DerefMut)]
pub struct EventSubscriber<S: EventStream> {
    #[deref]
    #[deref_mut]
    subscriber: TypedStreamSubscriber<S::Message>,
}

impl<S: EventStream> EventSubscriber<S> {
    /// Create a new event subscriber using the stream's default consumer name.
    ///
    /// If the stream doesn't exist, it will be created with the configuration
    /// from the `EventStream` trait.
    pub(crate) async fn new(jetstream: &Context) -> Result<Self> {
        let subscriber = TypedStreamSubscriber::new_with_max_age(
            jetstream,
            S::NAME,
            S::CONSUMER_NAME,
            S::MAX_AGE,
            S::ACK_WAIT,
            S::MAX_DELIVER,
            Some(format!("{}.>", S::NAME)),
        )
        .await?;
        Ok(Self { subscriber })
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

    /// Returns the consumer name.
    #[inline]
    pub fn consumer_name(&self) -> &'static str {
        S::CONSUMER_NAME
    }
}
