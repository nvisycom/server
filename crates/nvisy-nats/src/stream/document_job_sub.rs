//! Document job stream subscriber.

use derive_more::{Deref, DerefMut};

use super::document_job::DocumentJob;
use super::subscriber::StreamSubscriber;
use crate::Result;

/// Document job subscriber wrapping the base StreamSubscriber
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct DocumentJobSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<DocumentJob>,
}

impl DocumentJobSubscriber {
    /// Create a new document job subscriber
    pub async fn new(
        jetstream: &async_nats::jetstream::Context,
        consumer_name: &str,
    ) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "DOCUMENT_JOBS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

// Re-export the stream and message types from base subscriber
pub use super::subscriber::{
    TypedBatchStream as DocumentJobBatchStream, TypedMessage as DocumentJobMessage,
    TypedMessageStream as DocumentJobStream,
};
