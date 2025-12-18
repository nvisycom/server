//! Project import job stream subscriber.

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::project_import::ProjectImportJob;
use super::subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
use crate::Result;

/// Project import job subscriber wrapping the base StreamSubscriber.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ProjectImportSubscriber {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<ProjectImportJob>,
}

impl ProjectImportSubscriber {
    /// Create a new project import job subscriber.
    pub async fn new(jetstream: &Context, consumer_name: &str) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, "PROJECT_IMPORTS", consumer_name).await?;
        Ok(Self { subscriber })
    }
}

/// Type alias for project import batch stream.
pub type ProjectImportBatchStream = TypedBatchStream<ProjectImportJob>;

/// Type alias for project import message.
pub type ProjectImportMessage = TypedMessage<ProjectImportJob>;

/// Type alias for project import message stream.
pub type ProjectImportStream = TypedMessageStream<ProjectImportJob>;
