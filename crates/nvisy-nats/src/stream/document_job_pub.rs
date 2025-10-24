//! Document job stream publisher.

use derive_more::{Deref, DerefMut};

use super::document_job::DocumentJob;
use super::publisher::StreamPublisher;
use crate::Result;

/// Document job publisher wrapping the base StreamPublisher
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct DocumentJobPublisher {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<DocumentJob>,
}

impl DocumentJobPublisher {
    /// Create a new document job publisher
    pub async fn new(jetstream: &async_nats::jetstream::Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, "DOCUMENT_JOBS").await?;
        Ok(Self { publisher })
    }
}
