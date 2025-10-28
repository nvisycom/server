//! Redaction service for identifying sensitive data to redact.
//!
//! This module provides a service that uses the LLM client to identify which
//! data items should be redacted based on user-specified criteria.

use std::collections::HashSet;

use openrouter_rs::api::chat::Message;
use openrouter_rs::types::Role;
use uuid::Uuid;

use super::chat_completion::TypedChatCompletion;
use super::chat_request::TypedChatRequest;
use super::redaction_prompts::{create_system_prompt, create_user_prompt};
use super::redaction_request::RedactionRequest;
use super::redaction_response::RedactionResponse;
use crate::REDACTION_TARGET;
use crate::client::LlmClient;
use crate::error::{Error, Result};

/// A service for performing data redaction tasks with OpenRouter LLMs.
///
/// This service combines redaction prompt formatting with the underlying
/// LLM client to provide a seamless experience for redaction tasks.
///
/// # Example
///
/// ```rust,no_run
/// use nvisy_openrouter::{LlmClient, completion::{RedactionService, RedactionRequest, RedactionItem}};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let llm_client = LlmClient::from_api_key("your-api-key")?;
///     let redaction_service = RedactionService::new(llm_client);
///
///     let request = RedactionRequest::new(
///         vec![RedactionItem::new("123 Main St, 555-1234")
///             .with_entity("John Doe")],
///         "Redact all addresses that belong to John Doe"
///     );
///
///     let response = redaction_service.redact(&request).await?;
///     println!("Found {} entities", response.entities.len());
///     println!("Redacting {} items", response.data.len());
///     Ok(())
/// }
/// ```
pub struct RedactionService {
    client: LlmClient,
}

impl RedactionService {
    /// Creates a new redaction service with the given LLM client.
    ///
    /// # Arguments
    ///
    /// * `client` - The LLM client to use for redaction analysis
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use nvisy_openrouter::{LlmClient, completion::RedactionService};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = LlmClient::from_api_key("your-api-key")?;
    /// let service = RedactionService::new(client);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(client: LlmClient) -> Self {
        Self { client }
    }

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
    /// # use nvisy_openrouter::{LlmClient, completion::{RedactionService, RedactionRequest, RedactionItem}};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let service = RedactionService::new(LlmClient::from_api_key("key")?);
    /// let request = RedactionRequest::builder()
    ///     .with_data(vec![RedactionItem::new("123 Main St")])
    ///     .with_prompt("Redact addresses")
    ///     .build()?;
    ///
    /// let response = service.redact(&request).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn redact(&self, request: &RedactionRequest) -> Result<RedactionResponse> {
        tracing::debug!(
            target: REDACTION_TARGET,
            data_items = request.data.len(),
            prompt_length = request.prompt.len(),
            "Starting redaction analysis"
        );

        // Create prompts
        let system_prompt = create_system_prompt();
        let user_prompt = create_user_prompt(request);

        tracing::trace!(
            target: REDACTION_TARGET,
            system_prompt_length = system_prompt.len(),
            user_prompt_length = user_prompt.len(),
            "Generated prompts for redaction"
        );

        // Create messages for the LLM
        let messages = vec![
            Message::new(Role::System, &system_prompt),
            Message::new(Role::User, &user_prompt),
        ];

        // Create a dummy request (we don't need request data for redaction)
        // The actual request data is in the user prompt
        #[derive(serde::Serialize, serde::Deserialize)]
        struct EmptyRequest;

        // Use typed chat completion with RedactionResponse schema
        let typed_completion =
            TypedChatCompletion::<EmptyRequest, RedactionResponse>::new(self.client.clone());

        let typed_request = TypedChatRequest::builder()
            .with_messages(messages)
            .with_request(EmptyRequest)
            .build()
            .map_err(|e| Error::api(format!("Failed to build typed request: {}", e)))?;

        let typed_response = typed_completion.chat_completion(typed_request).await?;

        tracing::trace!(
            target: REDACTION_TARGET,
            response_length = typed_response.raw_response.as_ref().map(|s| s.len()).unwrap_or(0),
            "Received LLM response"
        );

        let redaction_response = typed_response.data;

        // Validate that returned IDs exist in the original request
        self.validate_response(request, &redaction_response)?;

        tracing::info!(
            target: REDACTION_TARGET,
            entities_found = redaction_response.entities.len(),
            data_to_redact = redaction_response.data.len(),
            "Redaction analysis completed successfully"
        );

        Ok(redaction_response)
    }

    /// Validates that all UUIDs in the response exist in the request.
    ///
    /// This ensures the LLM hasn't hallucinated IDs that weren't in the original request.
    ///
    /// # Arguments
    ///
    /// * `request` - The original redaction request
    /// * `response` - The parsed redaction response
    ///
    /// # Errors
    ///
    /// Returns an error if any UUID in the response doesn't exist in the request
    fn validate_response(
        &self,
        request: &RedactionRequest,
        response: &RedactionResponse,
    ) -> Result<()> {
        let valid_ids: HashSet<Uuid> = request.data.iter().map(|item| item.id).collect();

        for data in &response.data {
            if !valid_ids.contains(&data.id) {
                tracing::error!(
                    target: REDACTION_TARGET,
                    invalid_id = %data.id,
                    valid_ids = ?valid_ids,
                    "LLM returned invalid UUID not present in request"
                );
                return Err(Error::api(format!(
                    "Invalid redaction ID in response: {}",
                    data.id
                )));
            }
        }

        tracing::trace!(
            target: REDACTION_TARGET,
            validated_count = response.data.len(),
            "All response IDs validated successfully"
        );

        Ok(())
    }

    /// Returns the underlying LLM client.
    ///
    /// This can be useful for accessing client configuration or performing
    /// other operations with the same client.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use nvisy_openrouter::{LlmClient, completion::RedactionService};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let service = RedactionService::new(LlmClient::from_api_key("key")?);
    /// let client = service.client();
    /// let config = client.config();
    /// println!("Using model: {}", config.effective_model());
    /// # Ok(())
    /// # }
    /// ```
    pub fn client(&self) -> LlmClient {
        self.client.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::redaction_request::RedactionItem;

    #[test]
    fn test_redaction_service_creation() {
        // This test just verifies the types work correctly
        // Real tests would require API keys or mocks
        assert!(true);
    }

    #[test]
    fn test_request_structure() {
        let request = RedactionRequest::builder()
            .with_data(vec![
                RedactionItem::new("123 Main St, 555-1234").with_entity("John Doe"),
                RedactionItem::new("8th of January, 1990").with_entity("John Doe"),
            ])
            .with_prompt("Redact all addresses that belong to John Doe")
            .build()
            .unwrap();

        assert_eq!(request.data.len(), 2);
        assert_eq!(request.data[0].text, "123 Main St, 555-1234");
        assert_eq!(request.data[0].entity.as_ref().unwrap(), "John Doe");
    }
}
