//! Typed stream over a core-NATS (non-JetStream) subscription.

use std::fmt::Display;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_nats::Subscriber;
use futures::Stream;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A core-NATS (non-JetStream) broadcast topic: the event it carries and the
/// subject an event about one resource is published on.
///
/// Broadcasts fan out live updates to whichever subscribers are connected; the
/// topic's source of truth lives elsewhere, so a missed event is recoverable.
pub trait BroadcastTopic: 'static {
    /// The event broadcast on this topic.
    type Event: Serialize + DeserializeOwned + Send + 'static;

    /// The subject `id`'s events are broadcast on.
    fn subject(id: &dyn Display) -> String;
}

/// A typed stream of messages from a core-NATS subject subscription.
///
/// Wraps a raw [`Subscriber`] and yields each payload deserialized into `T`.
/// Messages that fail to deserialize are skipped rather than ending the stream.
/// The stream ends when the underlying subscription is dropped or unsubscribed.
#[must_use = "streams do nothing unless polled"]
pub struct BroadcastStream<T> {
    subscriber: Subscriber,
    _marker: PhantomData<fn() -> T>,
}

impl<T> BroadcastStream<T> {
    /// Wraps a raw subscriber into a typed broadcast stream.
    pub(crate) fn new(subscriber: Subscriber) -> Self {
        Self {
            subscriber,
            _marker: PhantomData,
        }
    }
}

impl<T> Stream for BroadcastStream<T>
where
    T: DeserializeOwned,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.subscriber).poll_next(cx) {
                Poll::Ready(Some(message)) => {
                    // Skip messages that fail to deserialize; a malformed
                    // broadcast should not tear down the subscription.
                    if let Ok(value) = serde_json::from_slice::<T>(&message.payload) {
                        return Poll::Ready(Some(value));
                    }
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
