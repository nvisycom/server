//! Document job stream subscriber.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::document_job::DocumentJob;
use super::subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
use crate::Result;

/// Document job subscriber wrapping the base StreamSubscriber.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct DocumentJobSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<DocumentJob>,
}

impl DocumentJobSubscriber {
    /// Create a new document job subscriber.
    pub async fn new(jetstream: &Context, consumer_name: &str) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "DOCUMENT_JOBS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

/// Type alias for document job batch stream.
pub type DocumentJobBatchStream = TypedBatchStream<DocumentJob>;

/// Type alias for document job message.
pub type DocumentJobMessage = TypedMessage<DocumentJob>;

/// Type alias for document job message stream.
pub type DocumentJobStream = TypedMessageStream<DocumentJob>;
