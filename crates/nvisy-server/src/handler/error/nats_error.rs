//! NATS error to HTTP error conversion implementation.
//!
//! This module provides conversion from NATS client errors to appropriate HTTP errors
//! with proper status codes and user-friendly messages.

use super::http_error::{Error as HttpError, ErrorKind};

impl<'a> From<nvisy_nats::Error> for HttpError<'a> {
    fn from(nats_error: nvisy_nats::Error) -> Self {
        match nats_error {
            // Connection and network errors -> Service Unavailable or Internal Server Error
            nvisy_nats::Error::Connection(_) => ErrorKind::InternalServerError
                .with_message("Service temporarily unavailable")
                .with_context("Unable to connect to messaging service"),

            nvisy_nats::Error::Timeout { .. } => ErrorKind::InternalServerError
                .with_message("Request timed out")
                .with_context("Operation took too long to complete"),

            nvisy_nats::Error::DeliveryFailed { ref subject, .. } => ErrorKind::InternalServerError
                .with_message("Message delivery failed")
                .with_context(format!("Failed to deliver message to {}", subject)),

            // Data validation and serialization errors -> Bad Request
            nvisy_nats::Error::Serialization(_) => ErrorKind::BadRequest
                .with_message("Invalid request or response data format")
                .with_context("Failed to serialize data for storage"),

            nvisy_nats::Error::InvalidConfig { .. } => ErrorKind::BadRequest
                .with_message("Invalid configuration")
                .with_context("Service configuration is invalid"),

            // Not found errors -> Not Found
            nvisy_nats::Error::KvKeyNotFound {
                ref key,
                ref bucket,
            } => ErrorKind::NotFound
                .with_message("Resource not found")
                .with_resource(key.clone())
                .with_context(format!("Key not found in storage bucket '{}'", bucket)),

            nvisy_nats::Error::KvBucketNotFound { ref bucket } => ErrorKind::NotFound
                .with_message("Storage bucket not found")
                .with_resource(bucket.clone())
                .with_context("The requested storage bucket does not exist"),

            nvisy_nats::Error::ObjectNotFound {
                ref name,
                ref bucket,
            } => ErrorKind::NotFound
                .with_message("Object not found")
                .with_resource(name.clone())
                .with_context(format!("Object not found in bucket '{}'", bucket)),

            nvisy_nats::Error::ObjectBucketNotFound { ref bucket } => ErrorKind::NotFound
                .with_message("Object storage bucket not found")
                .with_resource(bucket.clone())
                .with_context("The requested object storage bucket does not exist"),

            // Revision conflicts -> Conflict
            nvisy_nats::Error::KvRevisionMismatch { ref key, .. } => ErrorKind::Conflict
                .with_message("Resource has been modified")
                .with_resource(key.clone())
                .with_context("The resource was modified by another request"),

            // JetStream and streaming errors -> Internal Server Error
            nvisy_nats::Error::JetstreamPublish(_) => ErrorKind::InternalServerError
                .with_message("Failed to publish message")
                .with_context("Unable to publish to JetStream"),

            nvisy_nats::Error::JetstreamMessage(_) => ErrorKind::InternalServerError
                .with_message("Message processing failed")
                .with_context("Error processing JetStream message"),

            nvisy_nats::Error::StreamError { ref stream, .. } => ErrorKind::InternalServerError
                .with_message("Stream operation failed")
                .with_resource(stream.clone())
                .with_context("Error accessing or manipulating stream"),

            nvisy_nats::Error::ConsumerError { ref consumer, .. } => ErrorKind::InternalServerError
                .with_message("Consumer operation failed")
                .with_resource(consumer.clone())
                .with_context("Error with message consumer"),

            nvisy_nats::Error::JobQueueError { ref queue, .. } => ErrorKind::InternalServerError
                .with_message("Job processing failed")
                .with_resource(queue.clone())
                .with_context("Error processing job from queue"),

            // Legacy streaming errors -> Internal Server Error
            nvisy_nats::Error::Consumer(_) => ErrorKind::InternalServerError
                .with_message("Consumer error")
                .with_context("Message consumer encountered an error"),

            nvisy_nats::Error::Stream(_) => ErrorKind::InternalServerError
                .with_message("Stream error")
                .with_context("Stream operation encountered an error"),

            nvisy_nats::Error::Ack(_) => ErrorKind::InternalServerError
                .with_message("Message acknowledgment failed")
                .with_context("Unable to acknowledge message receipt"),

            // Generic operation error -> Internal Server Error
            nvisy_nats::Error::Operation { ref operation, .. } => ErrorKind::InternalServerError
                .with_message(format!("Operation '{}' failed", operation))
                .with_context("The requested operation could not be completed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_connection_error_conversion() {
        let nats_err = nvisy_nats::Error::timeout(Duration::from_secs(30));
        let http_err: HttpError = nats_err.into();

        assert_eq!(http_err.kind(), ErrorKind::InternalServerError);
        assert!(http_err.message().unwrap().contains("timed out"));
    }

    #[test]
    fn test_serialization_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let nats_err = nvisy_nats::Error::Serialization(json_err);
        let http_err: HttpError = nats_err.into();

        assert_eq!(http_err.kind(), ErrorKind::BadRequest);
        assert!(http_err.message().unwrap().contains("Invalid request data"));
    }

    #[test]
    fn test_not_found_error_conversion() {
        let nats_err = nvisy_nats::Error::kv_key_not_found("test_bucket", "missing_key");
        let http_err: HttpError = nats_err.into();

        assert_eq!(http_err.kind(), ErrorKind::NotFound);
        assert_eq!(http_err.resource(), Some("missing_key"));
        assert!(http_err.context().unwrap().contains("test_bucket"));
    }

    #[test]
    fn test_revision_mismatch_conversion() {
        let nats_err = nvisy_nats::Error::kv_revision_mismatch("test_key", 5, 7);
        let http_err: HttpError = nats_err.into();

        assert_eq!(http_err.kind(), ErrorKind::Conflict);
        assert_eq!(http_err.resource(), Some("test_key"));
        assert!(http_err.context().unwrap().contains("Expected revision 5"));
    }

    #[test]
    fn test_stream_error_conversion() {
        let nats_err = nvisy_nats::Error::stream_error("test_stream", "stream not available");
        let http_err: HttpError = nats_err.into();

        assert_eq!(http_err.kind(), ErrorKind::InternalServerError);
        assert_eq!(http_err.resource(), Some("test_stream"));
        assert!(http_err.context().unwrap().contains("stream not available"));
    }

    #[test]
    fn test_object_not_found_conversion() {
        let nats_err = nvisy_nats::Error::object_not_found("files", "document.pdf");
        let http_err: HttpError = nats_err.into();

        assert_eq!(http_err.kind(), ErrorKind::NotFound);
        assert_eq!(http_err.resource(), Some("document.pdf"));
        assert!(http_err.context().unwrap().contains("files"));
    }

    #[test]
    fn test_invalid_config_conversion() {
        let nats_err = nvisy_nats::Error::invalid_config("missing server URL");
        let http_err: HttpError = nats_err.into();

        assert_eq!(http_err.kind(), ErrorKind::BadRequest);
        assert!(http_err.context().unwrap().contains("missing server URL"));
    }
}
