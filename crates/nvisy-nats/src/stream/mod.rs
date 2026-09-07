//! JetStream streams for real-time updates and distributed job processing.
//!
//! This module provides type-safe streaming capabilities: generic event
//! publishing and subscribing over a stream configured via [`EventStream`].

mod broadcast_stream;
mod core;
mod event_stream;
mod typed_pub;
mod typed_stream;
mod typed_sub;

pub use core::EventStream;

pub use broadcast_stream::BroadcastStream;
pub use event_stream::{ConnectionSyncStream, DetectionStream, WebhookStream};
pub use typed_pub::EventPublisher;
pub use typed_stream::{TypedMessage, TypedMessageStream};
pub use typed_sub::EventSubscriber;
