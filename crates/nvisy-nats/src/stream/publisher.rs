//! Event publishing for real-time updates via WebSocket.

use std::collections::HashMap;

use async_nats::jetstream::{self, stream};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::{Error, Result, TRACING_TARGET_STREAM};

/// Stream publisher for real-time updates
pub struct StreamPublisher {
    jetstream: jetstream::Context,
    stream_name: String,
}

impl StreamPublisher {
    /// Create a new stream publisher
    #[instrument(skip(jetstream), target = TRACING_TARGET_STREAM)]
    pub async fn new(jetstream: &jetstream::Context, stream_name: &str) -> Result<Self> {
        let stream_config = stream::Config {
            name: stream_name.to_string(),
            description: Some(format!("Real-time updates stream: {}", stream_name)),
            subjects: vec![format!("updates.{}.>", stream_name)],
            max_age: std::time::Duration::from_secs(3600), // Keep messages for 1 hour
            ..Default::default()
        };

        // Try to get existing stream first
        match jetstream.get_stream(stream_name).await {
            Ok(_) => {
                debug!(
                    target: TRACING_TARGET_STREAM,
                    stream = %stream_name,
                    "Using existing stream"
                );
            }
            Err(_) => {
                // Stream doesn't exist, create it
                debug!(
                    target: TRACING_TARGET_STREAM,
                    stream = %stream_name,
                    max_age_secs = 3600,
                    "Creating new stream"
                );
                jetstream
                    .create_stream(stream_config)
                    .await
                    .map_err(|e| Error::operation("stream_create", e.to_string()))?;
            }
        }

        Ok(Self {
            jetstream: jetstream.clone(),
            stream_name: stream_name.to_string(),
        })
    }

    /// Publish an update event
    #[instrument(skip(self, event), target = TRACING_TARGET_STREAM)]
    pub async fn publish(&self, event: &UpdateEvent) -> Result<()> {
        let subject = self.generate_subject(&event.update_type);
        let payload = serde_json::to_vec(event)?;
        let payload_size = payload.len();

        self.jetstream
            .publish(subject.clone(), payload.into())
            .await
            .map_err(|e| Error::delivery_failed(&subject, e.to_string()))?
            .await
            .map_err(|e| Error::operation("stream_publish", e.to_string()))?;

        debug!(
            target: TRACING_TARGET_STREAM,
            subject = %subject,
            event_id = %event.event_id,
            payload_size = payload_size,
            "Published update event"
        );
        Ok(())
    }

    /// Publish multiple events in batch
    #[instrument(skip(self, events), target = TRACING_TARGET_STREAM)]
    pub async fn publish_batch(&self, events: &[UpdateEvent]) -> Result<()> {
        let count = events.len();
        for event in events {
            self.publish(event).await?;
        }

        debug!(
            target: TRACING_TARGET_STREAM,
            count = count,
            stream = %self.stream_name,
            "Published batch of events"
        );
        Ok(())
    }

    /// Generate subject based on update type
    fn generate_subject(&self, update_type: &UpdateType) -> String {
        match update_type {
            UpdateType::DocumentProgress { document_id, .. } => {
                format!(
                    "updates.{}.document.{}.progress",
                    self.stream_name, document_id
                )
            }
            UpdateType::DocumentComplete { document_id, .. } => {
                format!(
                    "updates.{}.document.{}.complete",
                    self.stream_name, document_id
                )
            }
            UpdateType::DocumentError { document_id, .. } => {
                format!(
                    "updates.{}.document.{}.error",
                    self.stream_name, document_id
                )
            }
            UpdateType::UserNotification { user_id, .. } => {
                format!("updates.{}.user.{}.notification", self.stream_name, user_id)
            }
            UpdateType::SystemAlert { .. } => {
                format!("updates.{}.system.alert", self.stream_name)
            }
            UpdateType::Custom { event_type, .. } => {
                format!("updates.{}.custom.{}", self.stream_name, event_type)
            }
        }
    }

    /// Create a durable consumer for processing updates
    #[instrument(skip(self), target = TRACING_TARGET_STREAM)]
    pub async fn create_consumer(
        &self,
        consumer_name: &str,
        filter_subject: Option<&str>,
    ) -> Result<jetstream::consumer::PullConsumer> {
        let mut consumer_config = jetstream::consumer::pull::Config {
            name: Some(consumer_name.to_string()),
            durable_name: Some(consumer_name.to_string()),
            description: Some(format!("Consumer for {}", consumer_name)),
            ..Default::default()
        };

        if let Some(subject) = filter_subject {
            consumer_config.filter_subject = subject.to_string();
        }

        let stream = self
            .jetstream
            .get_stream(&self.stream_name)
            .await
            .map_err(|e| Error::stream_error(&self.stream_name, e.to_string()))?;

        let consumer = stream
            .create_consumer(consumer_config)
            .await
            .map_err(|e| Error::consumer_error(consumer_name, e.to_string()))?;

        debug!(
            target: TRACING_TARGET_STREAM,
            consumer = %consumer_name,
            stream = %self.stream_name,
            filter_subject = ?filter_subject,
            "Created durable consumer"
        );
        Ok(consumer)
    }
}

/// Update event for real-time notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEvent {
    pub event_id: Uuid,
    pub timestamp: Timestamp,
    pub update_type: UpdateType,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl UpdateEvent {
    /// Create a new update event
    pub fn new(update_type: UpdateType) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Timestamp::now(),
            update_type,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the event
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Types of updates that can be sent via WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum UpdateType {
    /// Document processing progress update
    DocumentProgress {
        document_id: Uuid,
        user_id: Uuid,
        percentage: u8,
        stage: String,
        estimated_completion: Option<Timestamp>,
    },

    /// Document processing completed
    DocumentComplete {
        document_id: Uuid,
        user_id: Uuid,
        processing_time_ms: u64,
        result_summary: String,
    },

    /// Document processing error
    DocumentError {
        document_id: Uuid,
        user_id: Uuid,
        error_message: String,
        retry_count: u32,
    },

    /// User notification
    UserNotification {
        user_id: Uuid,
        notification_id: Uuid,
        title: String,
        message: String,
        priority: NotificationPriority,
        action_url: Option<String>,
    },

    /// System-wide alert
    SystemAlert {
        alert_type: String,
        message: String,
        severity: AlertSeverity,
        affected_users: Option<Vec<Uuid>>,
    },

    /// Custom event type
    Custom {
        event_type: String,
        payload: serde_json::Value,
    },
}

/// Notification priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_event_creation() {
        let doc_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let event = UpdateEvent::new(UpdateType::DocumentProgress {
            document_id: doc_id,
            user_id,
            percentage: 50,
            stage: "OCR Processing".to_string(),
            estimated_completion: None,
        });

        assert!(!event.event_id.is_nil());
        assert!(matches!(
            event.update_type,
            UpdateType::DocumentProgress { .. }
        ));
    }

    #[test]
    fn test_update_event_with_metadata() {
        let event = UpdateEvent::new(UpdateType::SystemAlert {
            alert_type: "maintenance".to_string(),
            message: "Scheduled maintenance".to_string(),
            severity: AlertSeverity::Warning,
            affected_users: None,
        })
        .with_metadata("duration".to_string(), serde_json::json!("30m"));

        assert!(event.metadata.contains_key("duration"));
    }
}
