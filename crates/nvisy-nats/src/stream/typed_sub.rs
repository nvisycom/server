//! Type-safe JetStream subscriber.
//!
//! A stream selects both its configuration and its payload type (see
//! [`EventStream`]), so [`EventSubscriber`] is generic over `S` alone; the
//! message type is `S::Message`. It consumes the stream through a durable pull
//! consumer as a [`TypedMessageStream`].

use std::marker::PhantomData;
use std::sync::Arc;

use async_nats::jetstream::{Context, consumer};

use super::core::{EventStream, ensure_stream};
use super::typed_stream::TypedMessageStream;
use crate::{Error, Result, TRACING_TARGET_STREAM};

/// Inner data shared by clones of an [`EventSubscriber`].
#[derive(Debug)]
struct EventSubscriberInner {
    jetstream: Context,
}

/// Consumes a stream's typed events through its durable pull consumer.
#[derive(Debug, Clone)]
pub struct EventSubscriber<S: EventStream> {
    inner: Arc<EventSubscriberInner>,
    _stream: PhantomData<fn() -> S>,
}

impl<S: EventStream> EventSubscriber<S> {
    /// Create a subscriber for the stream, creating the stream if it is absent.
    pub(crate) async fn new(jetstream: &Context) -> Result<Self> {
        ensure_stream::<S>(jetstream).await?;
        Ok(Self {
            inner: Arc::new(EventSubscriberInner {
                jetstream: jetstream.clone(),
            }),
            _stream: PhantomData,
        })
    }

    /// Open (creating or updating) the stream's durable pull consumer and return
    /// a typed message stream over it.
    ///
    /// `create_consumer` upserts, so a changed `ACK_WAIT`/`MAX_DELIVER` is applied
    /// to an existing consumer rather than silently ignored.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn subscribe(&self) -> Result<TypedMessageStream<S::Message>> {
        let mut config = consumer::pull::Config {
            durable_name: Some(S::CONSUMER_NAME.to_owned()),
            description: Some(format!("Consumer for stream {}", S::NAME)),
            ack_policy: consumer::AckPolicy::Explicit,
            filter_subject: format!("{}.>", S::NAME),
            ..Default::default()
        };
        if let Some(ack_wait) = S::ACK_WAIT {
            config.ack_wait = ack_wait;
        }
        if let Some(max_deliver) = S::MAX_DELIVER {
            config.max_deliver = max_deliver;
        }

        let stream = self
            .inner
            .jetstream
            .get_stream(S::NAME)
            .await
            .map_err(|e| Error::stream_error(S::NAME, format!("Failed to get stream: {e}")))?;

        let consumer = stream.create_consumer(config).await.map_err(|e| {
            Error::consumer_error(S::CONSUMER_NAME, format!("Failed to create consumer: {e}"))
        })?;

        tracing::debug!(
            target: TRACING_TARGET_STREAM,
            stream = %S::NAME,
            consumer = %S::CONSUMER_NAME,
            "Subscribed to stream"
        );
        Ok(TypedMessageStream::new(consumer))
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
