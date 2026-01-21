//! Generic event stream subscriber.

use std::marker::PhantomData;

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};
use serde::de::DeserializeOwned;

use super::event_stream::EventStream;
use super::stream_sub::StreamSubscriber;
use crate::Result;

/// Generic event subscriber for consuming typed events.
///
/// This subscriber is generic over:
/// - `T`: The event/message type to consume
/// - `S`: The stream configuration (determines stream name, subject, consumer name)
#[derive(Debug, Deref, DerefMut)]
pub struct EventSubscriber<T, S>
where
    T: DeserializeOwned + Send + Sync + 'static,
    S: EventStream,
{
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<T>,
    _stream: PhantomData<S>,
}

impl<T, S> EventSubscriber<T, S>
where
    T: DeserializeOwned + Send + Sync + 'static,
    S: EventStream,
{
    /// Create a new event subscriber using the stream's default consumer name.
    pub(crate) async fn new(jetstream: &Context) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, S::NAME, S::CONSUMER_NAME)
            .await?
            .with_filter_subject(format!("{}.>", S::NAME));
        Ok(Self {
            subscriber,
            _stream: PhantomData,
        })
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
