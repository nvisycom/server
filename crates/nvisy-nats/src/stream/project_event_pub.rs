//! Project event job stream publisher.

use derive_more::{Deref, DerefMut};

use super::project_event::ProjectEventJob;
use super::publisher::StreamPublisher;
use crate::Result;

/// Project event job publisher wrapping the base StreamPublisher
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectEventPublisher {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<ProjectEventJob>,
}

impl ProjectEventPublisher {
    /// Create a new project event job publisher
    pub async fn new(jetstream: &async_nats::jetstream::Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, "PROJECT_EVENTS").await?;
        Ok(Self { publisher })
    }
}
