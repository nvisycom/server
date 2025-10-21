//! Job queue management and worker processing.

use std::time::Duration;

use async_nats::jetstream::{self, stream};
use futures::StreamExt;
use jiff::Timestamp;
use serde_json;
use tracing::{debug, error, instrument, warn};

use super::job::{Job, JobStatus, JobType};
use crate::{Error, Result, TRACING_TARGET_QUEUE};

/// Job queue for distributed job processing
pub struct JobQueue {
    jetstream: jetstream::Context,
    stream_name: String,
    worker_id: String,
}

impl JobQueue {
    /// Create a new job queue
    #[instrument(skip(jetstream), target = TRACING_TARGET_QUEUE)]
    pub async fn new(
        jetstream: &jetstream::Context,
        queue_name: &str,
        worker_id: &str,
    ) -> Result<Self> {
        let stream_name = format!("JOBS_{}", queue_name.to_uppercase());

        let stream_config = stream::Config {
            name: stream_name.clone(),
            description: Some(format!("Job queue: {}", queue_name)),
            subjects: vec![format!("jobs.{}.>", queue_name)],
            retention: stream::RetentionPolicy::WorkQueue,
            ..Default::default()
        };

        // Try to get existing stream first
        match jetstream.get_stream(&stream_name).await {
            Ok(_) => {
                debug!(
                    target: TRACING_TARGET_QUEUE,
                    stream = %stream_name,
                    worker_id = %worker_id,
                    "Using existing job stream"
                );
            }
            Err(_) => {
                // Stream doesn't exist, create it
                debug!(
                    target: TRACING_TARGET_QUEUE,
                    stream = %stream_name,
                    worker_id = %worker_id,
                    queue_name = %queue_name,
                    "Creating new job stream"
                );
                jetstream
                    .create_stream(stream_config)
                    .await
                    .map_err(|e| Error::operation("stream_create", e.to_string()))?;
            }
        }

        Ok(Self {
            jetstream: jetstream.clone(),
            stream_name,
            worker_id: worker_id.to_string(),
        })
    }

    /// Submit a job to the queue
    #[instrument(skip(self, job), target = TRACING_TARGET_QUEUE)]
    pub async fn submit(&self, job: &Job) -> Result<()> {
        let subject = self.generate_subject(&job.job_type, job.priority.as_num());
        let payload = serde_json::to_vec(job)?;

        self.jetstream
            .publish(subject.clone(), payload.into())
            .await
            .map_err(|e| Error::delivery_failed(&subject, e.to_string()))?
            .await
            .map_err(|e| Error::operation("job_submit", e.to_string()))?;

        debug!(
            target: TRACING_TARGET_QUEUE,
            job_id = %job.id,
            job_type = %job.job_type,
            priority = job.priority.as_num(),
            subject = %subject,
            "Submitted job to queue"
        );
        Ok(())
    }

    /// Submit multiple jobs in batch
    #[instrument(skip(self, jobs), target = TRACING_TARGET_QUEUE)]
    pub async fn submit_batch(&self, jobs: &[Job]) -> Result<()> {
        let count = jobs.len();
        for job in jobs {
            self.submit(job).await?;
        }

        debug!(
            target: TRACING_TARGET_QUEUE,
            count = count,
            worker_id = %self.worker_id,
            "Submitted batch of jobs"
        );
        Ok(())
    }

    /// Create a worker consumer for processing jobs
    #[instrument(skip(self), target = TRACING_TARGET_QUEUE)]
    pub async fn create_worker(
        &self,
        job_types: &[JobType],
    ) -> Result<jetstream::consumer::PullConsumer> {
        let consumer_name = format!("worker_{}", self.worker_id);

        let consumer_config = jetstream::consumer::pull::Config {
            name: Some(consumer_name.clone()),
            durable_name: Some(consumer_name.clone()),
            description: Some(format!("Worker {} job consumer", self.worker_id)),
            ack_wait: Duration::from_secs(300), // 5 minutes to process job
            max_deliver: 3,                     // Maximum redeliveries
            ..Default::default()
        };

        let stream = self
            .jetstream
            .get_stream(&self.stream_name)
            .await
            .map_err(|e| Error::stream_error(&self.stream_name, e.to_string()))?;

        let consumer = stream
            .create_consumer(consumer_config)
            .await
            .map_err(|e| Error::consumer_error(&consumer_name, e.to_string()))?;

        debug!(
            target: TRACING_TARGET_QUEUE,
            consumer = %consumer_name,
            worker_id = %self.worker_id,
            job_types = ?job_types,
            "Created worker consumer"
        );
        Ok(consumer)
    }

    /// Process the next job from the queue
    #[instrument(skip(self, consumer, handler), target = TRACING_TARGET_QUEUE)]
    pub async fn process_next<F, Fut>(
        &self,
        consumer: &jetstream::consumer::PullConsumer,
        handler: F,
    ) -> Result<bool>
    where
        F: FnOnce(Job) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value>>,
    {
        // Fetch one message
        let mut messages = consumer
            .fetch()
            .max_messages(1)
            .messages()
            .await
            .map_err(|e| Error::operation("job_fetch", e.to_string()))?;

        if let Some(Ok(msg)) = messages.next().await {
            // Deserialize job
            let mut job: Job = match serde_json::from_slice(&msg.payload) {
                Ok(j) => j,
                Err(e) => {
                    error!(
                        target: TRACING_TARGET_QUEUE,
                        error = %e,
                        worker_id = %self.worker_id,
                        "Failed to deserialize job"
                    );
                    // Ack the message to remove it from queue
                    msg.ack().await.ok();
                    return Ok(false);
                }
            };

            // Check if job is ready to execute
            if !job.is_ready() {
                // Job is scheduled for future, nack and let it be redelivered
                msg.ack_with(async_nats::jetstream::AckKind::Nak(Some(
                    Duration::from_secs(60),
                )))
                .await
                .ok();
                return Ok(false);
            }

            debug!(
                target: TRACING_TARGET_QUEUE,
                job_id = %job.id,
                job_type = %job.job_type,
                worker_id = %self.worker_id,
                "Processing job"
            );

            // Update job status to running
            job.status = JobStatus::Running {
                worker_id: self.worker_id.clone(),
                started_at: Timestamp::now(),
            };

            let start_time = std::time::Instant::now();

            // Execute job handler
            match handler(job.clone()).await {
                Ok(result) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    job.status = JobStatus::Completed {
                        completed_at: Timestamp::now(),
                        duration_ms,
                        result: Some(result),
                    };

                    debug!(
                        target: TRACING_TARGET_QUEUE,
                        job_id = %job.id,
                        job_type = %job.job_type,
                        duration_ms = duration_ms,
                        worker_id = %self.worker_id,
                        "Job completed successfully"
                    );

                    // Ack the message
                    msg.ack()
                        .await
                        .map_err(|e| Error::operation("job_ack", e.to_string()))?;

                    Ok(true)
                }
                Err(e) => {
                    error!(
                        target: TRACING_TARGET_QUEUE,
                        job_id = %job.id,
                        job_type = %job.job_type,
                        error = %e,
                        worker_id = %self.worker_id,
                        "Job failed"
                    );

                    job.increment_retry();

                    if job.can_retry() {
                        // Nack the message for retry
                        warn!(
                            target: TRACING_TARGET_QUEUE,
                            job_id = %job.id,
                            retry_count = job.retry_count,
                            max_retries = job.max_retries,
                            worker_id = %self.worker_id,
                            "Job failed, will retry"
                        );

                        msg.ack_with(async_nats::jetstream::AckKind::Nak(Some(
                            Duration::from_secs(10 * job.retry_count as u64),
                        )))
                        .await
                        .ok();
                    } else {
                        // Max retries reached, mark as failed and ack
                        job.status = JobStatus::Failed {
                            failed_at: Timestamp::now(),
                            error: e.to_string(),
                            retry_count: job.retry_count,
                        };

                        error!(
                            target: TRACING_TARGET_QUEUE,
                            job_id = %job.id,
                            job_type = %job.job_type,
                            retry_count = job.retry_count,
                            worker_id = %self.worker_id,
                            "Job failed permanently after max retries"
                        );

                        msg.ack()
                            .await
                            .map_err(|e| Error::operation("job_ack", e.to_string()))?;
                    }

                    Ok(false)
                }
            }
        } else {
            // No messages available
            Ok(false)
        }
    }

    /// Generate subject for job based on type and priority
    fn generate_subject(&self, job_type: &JobType, priority: u8) -> String {
        format!(
            "jobs.{}.{}.priority_{}",
            self.extract_queue_name(),
            job_type,
            priority
        )
    }

    /// Extract queue name from stream name
    fn extract_queue_name(&self) -> String {
        self.stream_name
            .strip_prefix("JOBS_")
            .unwrap_or(&self.stream_name)
            .to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::job::{JobPriority, JobType};

    #[test]
    fn test_subject_generation() {
        let queue = JobQueue {
            jetstream: async_nats::jetstream::new(async_nats::Client::new()),
            stream_name: "JOBS_DOCUMENTS".to_string(),
            worker_id: "worker1".to_string(),
        };

        let subject =
            queue.generate_subject(&JobType::DocumentProcessing, JobPriority::High.as_num());
        assert_eq!(subject, "jobs.documents.document_processing.priority_2");
    }

    #[test]
    fn test_extract_queue_name() {
        let queue = JobQueue {
            jetstream: async_nats::jetstream::new(async_nats::Client::new()),
            stream_name: "JOBS_DOCUMENTS".to_string(),
            worker_id: "worker1".to_string(),
        };

        assert_eq!(queue.extract_queue_name(), "documents");
    }
}
