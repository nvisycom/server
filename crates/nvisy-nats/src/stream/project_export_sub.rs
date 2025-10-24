//! Project export job stream subscriber.

use derive_more::{Deref, DerefMut};

use super::project_export::ProjectExportJob;
use super::subscriber::StreamSubscriber;
use crate::Result;

/// Project export job subscriber wrapping the base StreamSubscriber
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectExportSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<ProjectExportJob>,
}

impl ProjectExportSubscriber {
    /// Create a new project export job subscriber
    pub async fn new(
        jetstream: &async_nats::jetstream::Context,
        consumer_name: &str,
    ) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_EXPORTS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

// Re-export the stream and message types from base subscriber
pub use super::subscriber::{
    TypedBatchStream as ProjectExportBatchStream, TypedMessage as ProjectExportMessage,
    TypedMessageStream as ProjectExportStream,
};
