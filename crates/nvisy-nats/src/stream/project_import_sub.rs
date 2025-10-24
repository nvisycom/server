//! Project import job stream subscriber.

use derive_more::{Deref, DerefMut};

use super::project_import::ProjectImportJob;
use super::subscriber::StreamSubscriber;
use crate::Result;

/// Project import job subscriber wrapping the base StreamSubscriber
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectImportSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<ProjectImportJob>,
}

impl ProjectImportSubscriber {
    /// Create a new project import job subscriber
    pub async fn new(
        jetstream: &async_nats::jetstream::Context,
        consumer_name: &str,
    ) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_IMPORTS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

// Re-export the stream and message types from base subscriber
pub use super::subscriber::{
    TypedBatchStream as ProjectImportBatchStream, TypedMessage as ProjectImportMessage,
    TypedMessageStream as ProjectImportStream,
};
