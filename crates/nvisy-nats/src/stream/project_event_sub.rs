//! Project event job stream subscriber.

use derive_more::{Deref, DerefMut};

use super::project_event::ProjectEventJob;
use super::subscriber::StreamSubscriber;
use crate::Result;

/// Project event job subscriber wrapping the base StreamSubscriber
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectEventSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<ProjectEventJob>,
}

impl ProjectEventSubscriber {
    /// Create a new project event job subscriber
    pub async fn new(
        jetstream: &async_nats::jetstream::Context,
        consumer_name: &str,
    ) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_EVENTS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

// Re-export the stream and message types from base subscriber
pub use super::subscriber::{
    TypedBatchStream as ProjectEventBatchStream, TypedMessage as ProjectEventMessage,
    TypedMessageStream as ProjectEventStream,
};
