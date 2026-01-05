//! Document job stream subscriber.

use std::marker::PhantomData;

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::document_job::{DocumentJob, STREAM_NAME, Stage};
use super::subscriber::StreamSubscriber;
use crate::Result;

/// Generic document job subscriber for a specific processing stage.
///
/// This subscriber filters jobs by stage-specific subjects within the
/// `DOCUMENT_JOBS` stream, enabling dedicated consumers per stage.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct DocumentJobSubscriber<S: Stage> {
    #[deref]
    #[deref_mut]
    subscriber: StreamSubscriber<DocumentJob<S>>,
    _marker: PhantomData<S>,
}

impl<S: Stage> DocumentJobSubscriber<S> {
    /// Create a new document job subscriber for the specified stage.
    ///
    /// The subscriber automatically filters to the stage-specific subject pattern.
    pub async fn new(jetstream: &Context, consumer_name: &str) -> Result<Self> {
        let filter_subject = format!("{}.{}.>", STREAM_NAME, S::SUBJECT);
        let subscriber = StreamSubscriber::new(jetstream, STREAM_NAME, consumer_name)
            .await?
            .with_filter_subject(filter_subject);
        Ok(Self {
            subscriber,
            _marker: PhantomData,
        })
    }

    /// Create a subscriber without stage filtering (receives all stages).
    ///
    /// Note: This requires the job type to match at deserialization time,
    /// so it's primarily useful for monitoring or debugging.
    pub async fn new_unfiltered(jetstream: &Context, consumer_name: &str) -> Result<Self> {
        let subscriber = StreamSubscriber::new(jetstream, STREAM_NAME, consumer_name).await?;
        Ok(Self {
            subscriber,
            _marker: PhantomData,
        })
    }

    /// Create a subscriber filtered to a specific file.
    pub async fn new_for_file(
        jetstream: &Context,
        consumer_name: &str,
        file_id: uuid::Uuid,
    ) -> Result<Self> {
        let filter_subject = format!("{}.{}.{}", STREAM_NAME, S::SUBJECT, file_id);
        let subscriber = StreamSubscriber::new(jetstream, STREAM_NAME, consumer_name)
            .await?
            .with_filter_subject(filter_subject);
        Ok(Self {
            subscriber,
            _marker: PhantomData,
        })
    }
}
