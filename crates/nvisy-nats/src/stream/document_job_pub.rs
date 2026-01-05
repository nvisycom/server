//! Document job stream publisher.

use std::marker::PhantomData;

use async_nats::jetstream::Context;
use derive_more::{Deref, DerefMut};

use super::document_job::{DocumentJob, STREAM_NAME, Stage};
use super::publisher::StreamPublisher;
use crate::Result;

/// Generic document job publisher for a specific processing stage.
///
/// This publisher routes jobs to stage-specific subjects within the
/// `DOCUMENT_JOBS` stream, enabling separate consumers per stage.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct DocumentJobPublisher<S: Stage> {
    #[deref]
    #[deref_mut]
    publisher: StreamPublisher<DocumentJob<S>>,
    _marker: PhantomData<S>,
}

impl<S: Stage> DocumentJobPublisher<S> {
    /// Create a new document job publisher for the specified stage.
    pub async fn new(jetstream: &Context) -> Result<Self> {
        let publisher = StreamPublisher::new(jetstream, STREAM_NAME).await?;
        Ok(Self {
            publisher,
            _marker: PhantomData,
        })
    }

    /// Publish a job to the stage-specific subject.
    ///
    /// Jobs are published to `DOCUMENT_JOBS.{stage}.{file_id}`.
    pub async fn publish_job(&self, job: &DocumentJob<S>) -> Result<()> {
        let subject = format!("{}.{}", S::SUBJECT, job.file_id);
        self.publisher.publish(&subject, job).await
    }

    /// Publish a job with a custom subject suffix.
    ///
    /// Jobs are published to `DOCUMENT_JOBS.{stage}.{suffix}`.
    pub async fn publish_job_with_subject(&self, job: &DocumentJob<S>, suffix: &str) -> Result<()> {
        let subject = format!("{}.{}", S::SUBJECT, suffix);
        self.publisher.publish(&subject, job).await
    }

    /// Publish multiple jobs in batch.
    pub async fn publish_batch(&self, jobs: &[DocumentJob<S>]) -> Result<()> {
        // Group by file_id isn't needed since we use the stage subject
        self.publisher
            .publish_batch_parallel(S::SUBJECT, jobs, 10)
            .await
    }
}
