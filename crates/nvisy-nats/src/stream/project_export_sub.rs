//! Project export job stream subscriber.

use derive_more::{Deref, DerefMut};

use super::project_export::ProjectExportJob;
use super::subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
use crate::Result;

/// Project export job subscriber wrapping the base StreamSubscriber.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectExportSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<ProjectExportJob>,
}

impl ProjectExportSubscriber {
    /// Create a new project export job subscriber.
    pub async fn new(
        jetstream: &async_nats::jetstream::Context,
        consumer_name: &str,
    ) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_EXPORTS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

/// Type alias for project export batch stream.
pub type ProjectExportBatchStream = TypedBatchStream<ProjectExportJob>;

/// Type alias for project export message.
pub type ProjectExportMessage = TypedMessage<ProjectExportJob>;

/// Type alias for project export message stream.
pub type ProjectExportStream = TypedMessageStream<ProjectExportJob>;
