//! Redaction service for identifying sensitive data to redact.
//!
//! This module provides a service that uses the LLM client to identify which
//! data items should be redacted based on user-specified criteria.

use std::collections::HashSet;

use portkey_sdk::model::{ChatCompletionRequest, ChatCompletionRequestMessage, ResponseFormat};
use uuid::Uuid;

use super::chat_completion::ChatCompletion;
use super::redaction_prompts::{create_system_prompt, create_user_prompt};
use super::redaction_request::RedactionRequest;
use super::redaction_response::RedactionResponse;
use crate::client::LlmClient;
use crate::{Error, Result, TRACING_TARGET_COMPLETION};

/// Trait for performing data redaction tasks with Portkey AI Gateway.
///
/// This trait provides methods for identifying sensitive data to redact
/// based on user-specified criteria.
///
/// # Example
///
/// ```rust,no_run
/// use nvisy_portkey::{LlmClient, completion::{RedactionService, RedactionRequest, RedactionItem}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = LlmClient::from_api_key("your-api-key")?;
///
/// let request = RedactionRequest::new(
///     vec![RedactionItem::new("123 Main St, 555-1234")
///         .with_entity("John Doe")],
///     "Redact all addresses that belong to John Doe"
/// );
///
/// let response = client.redact(&request).await?;
/// println!("Found {} entities", response.entities.len());
/// println!("Redacting {} items", response.data.len());
/// # Ok(())
/// # }
/// ```
pub trait RedactionService {
    /// Performs a redaction analysis on the given data.
    ///
    /// This method formats the redaction request, sends it to the LLM,
    /// and parses the response to extract entities and items to redact.
    ///
    /// # Arguments
    ///
    /// * `request` - The redaction request containing data items and criteria
    ///
    /// # Returns
    ///
    /// A `RedactionResponse` containing identified entities and data to redact
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The LLM API call fails
    /// - The LLM response cannot be parsed
    /// - The response contains invalid data
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use nvisy_portkey::{LlmClient, completion::{RedactionService, RedactionRequest, RedactionItem}};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = LlmClient::from_api_key("key")?;
    /// let request = RedactionRequest::builder()
    ///     .with_data(vec![RedactionItem::new("123 Main St")])
    ///     .with_prompt("Redact all addresses")
    ///     .build()?;
    ///
    /// let response = client.redact(&request).await?;
    /// # Ok(())
    /// # }
    /// ```
    fn redact(
        &self,
        request: &RedactionRequest,
    ) -> impl std::future::Future<Output = Result<RedactionResponse>> + Send;
}

impl RedactionService for LlmClient {
    async fn redact(&self, request: &RedactionRequest) -> Result<RedactionResponse> {
        tracing::debug!(
            target: TRACING_TARGET_COMPLETION,
            data_count = request.data.len(),
            "Starting redaction analysis"
        );

        // Create the prompts for the LLM
        let system_prompt = create_system_prompt();
        let user_prompt = create_user_prompt(request);

        // Build messages
        let messages = vec![
            ChatCompletionRequestMessage::System {
                content: system_prompt,
                name: None,
            },
            ChatCompletionRequestMessage::user(user_prompt),
        ];

        // Get model from config
        let model = self.as_config().effective_model().to_string();

        // Build the chat completion request with JSON Schema
        let mut chat_request = ChatCompletionRequest::new(model, messages);

        // Configure structured output using JSON Schema
        chat_request.response_format = Some(ResponseFormat::JsonSchema {
            json_schema: portkey_sdk::model::JsonSchema::from_type::<RedactionResponse>()
                .with_description("Redaction analysis response with identified entities and data")
                .with_strict(true),
        });

        // Execute the structured completion
        let mut context = super::ChatContext::empty();
        let response = self
            .structured_chat_completion::<RedactionResponse>(&mut context, chat_request)
            .await?
            .ok_or_else(|| Error::invalid_response("No redaction response received"))?;

        // Validate the response
        validate_response(request, &response)?;

        tracing::info!(
            target: TRACING_TARGET_COMPLETION,
            entity_count = response.entities.len(),
            redact_count = response.data.len(),
            "Redaction analysis completed"
        );

        Ok(response)
    }
}

/// Validates that the redaction response is consistent with the request.
///
/// Checks:
/// - All item IDs in the response exist in the request
/// - No duplicate item IDs in the response
fn validate_response(request: &RedactionRequest, response: &RedactionResponse) -> Result<()> {
    // Build a set of valid IDs from the request
    let valid_ids: HashSet<Uuid> = request.data.iter().map(|item| item.id).collect();

    // Check all response item IDs are valid
    let mut seen_ids = HashSet::new();
    for item in &response.data {
        if !valid_ids.contains(&item.id) {
            return Err(Error::invalid_response(format!(
                "Response contains unknown item ID: {}",
                item.id
            )));
        }

        if !seen_ids.insert(item.id) {
            return Err(Error::invalid_response(format!(
                "Response contains duplicate item ID: {}",
                item.id
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::{RedactedData, RedactionItem};

    #[test]
    fn test_validate_response_success() {
        let item1 = RedactionItem::new("test data 1");
        let item2 = RedactionItem::new("test data 2");

        let request = RedactionRequest::new(vec![item1.clone(), item2.clone()], "test criteria");

        let response = RedactionResponse {
            entities: vec![],
            data: vec![
                RedactedData::builder()
                    .with_id(item1.id)
                    .with_data("test data 1".to_string())
                    .build()
                    .unwrap(),
            ],
        };

        assert!(validate_response(&request, &response).is_ok());
    }

    #[test]
    fn test_validate_response_unknown_id() {
        let item = RedactionItem::new("test data");
        let request = RedactionRequest::new(vec![item], "test criteria");

        let response = RedactionResponse {
            entities: vec![],
            data: vec![
                RedactedData::builder()
                    .with_id(Uuid::new_v4()) // Random unknown ID
                    .with_data("test data".to_string())
                    .build()
                    .unwrap(),
            ],
        };

        assert!(validate_response(&request, &response).is_err());
    }

    #[test]
    fn test_validate_response_duplicate_id() {
        let item = RedactionItem::new("test data");
        let request = RedactionRequest::new(vec![item.clone()], "test criteria");

        let redacted = RedactedData::builder()
            .with_id(item.id)
            .with_data("test data".to_string())
            .build()
            .unwrap();

        let response = RedactionResponse {
            entities: vec![],
            data: vec![redacted.clone(), redacted], // Duplicate
        };

        assert!(validate_response(&request, &response).is_err());
    }
}
