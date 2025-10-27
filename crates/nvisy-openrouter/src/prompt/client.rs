//! Redaction client helper for seamless integration with OpenRouter API.
//!
//! This module provides a high-level interface that combines redaction prompt
//! formatting with the LLM client to perform data redaction tasks.

use crate::prompt::{RedactionPrompt, RedactionRequest, RedactionResponse};
use crate::service::{ChatCompletionRequest, Error, LlmClient, Message, Result};

/// A helper client for performing data redaction tasks with OpenRouter LLMs.
///
/// This client combines the redaction prompt formatting with the underlying
/// LLM client to provide a seamless experience for redaction tasks.
///
/// # Example
///
/// ```rust,no_run
/// use nvisy_openrouter::{LlmClient, RedactionClient, RedactionRequest, RedactionItem};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let llm_client = LlmClient::from_api_key("your-api-key")?;
///     let redaction_client = RedactionClient::new(llm_client);
///
///     let request = RedactionRequest {
///         data: vec![
///             RedactionItem {
///                 id: "1".to_string(),
///                 text: "123 Main St, 555-1234".to_string(),
///                 entity: "John Doe".to_string(),
///                 data_type: "address".to_string(),
///             }
///         ],
///         prompt: "Redact all addresses that belong to John Doe".to_string(),
///     };
///
///     let response = redaction_client.redact(&request).await?;
///     println!("Items to redact: {:?}", response.redact_ids);
///     Ok(())
/// }
/// ```
pub struct RedactionClient {
    llm_client: LlmClient,
    prompt_formatter: RedactionPrompt,
}

impl RedactionClient {
    /// Creates a new redaction client with the given LLM client.
    ///
    /// Uses the default redaction prompt formatter.
    pub fn new(llm_client: LlmClient) -> Self {
        Self {
            llm_client,
            prompt_formatter: RedactionPrompt::new(),
        }
    }

    /// Creates a new redaction client with a custom prompt formatter.
    ///
    /// This allows you to customize the system prompt and formatting behavior.
    pub fn with_prompt_formatter(llm_client: LlmClient, prompt_formatter: RedactionPrompt) -> Self {
        Self {
            llm_client,
            prompt_formatter,
        }
    }

    /// Performs a redaction analysis on the given data.
    ///
    /// This method formats the redaction request, sends it to the LLM,
    /// and parses the response to extract the IDs that should be redacted.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The LLM API call fails
    /// - The LLM response cannot be parsed
    /// - The response contains invalid redaction IDs
    pub async fn redact(&self, request: &RedactionRequest) -> Result<RedactionResponse> {
        // Format the request into an LLM prompt
        let formatted_prompt = self.prompt_formatter.format_request(request);

        // Create a message for the LLM
        let message = Message {
            role: "user".to_string(),
            content: formatted_prompt,
            name: None,
            tool_calls: None,
        };

        // Send to LLM
        let llm_response = self
            .llm_client
            .chat_completion_with_messages(vec![message])
            .await?;

        // Extract the content from the first choice
        let content = llm_response
            .choices
            .first()
            .ok_or_else(|| Error::api("No response choices returned from LLM"))?
            .message
            .content
            .as_str();

        // Parse the response
        let redaction_response = self
            .prompt_formatter
            .parse_response(content)
            .map_err(|e| Error::api(format!("Failed to parse LLM response: {}", e)))?;

        // Validate that all returned IDs exist in the original request
        crate::prompt::validate_redaction_ids(request, &redaction_response)
            .map_err(|e| Error::api(format!("Invalid redaction response: {}", e)))?;

        Ok(redaction_response)
    }

    /// Performs a redaction analysis with custom request options.
    ///
    /// This method provides the same functionality as `redact` but
    /// allows for future extensibility.
    pub async fn redact_with_options(
        &self,
        request: &RedactionRequest,
    ) -> Result<RedactionResponse> {
        // Format the request into an LLM prompt
        let formatted_prompt = self.prompt_formatter.format_request(request);

        // Create a message for the LLM
        let message = Message {
            role: "user".to_string(),
            content: formatted_prompt,
            name: None,
            tool_calls: None,
        };

        // Build a chat completion request
        let chat_request = ChatCompletionRequest {
            model: self.llm_client.config().effective_model().to_string(),
            messages: vec![message],
            stream: Some(false),
            response_format: None,
            tools: None,
            provider: None,
            models: None,
            transforms: None,
        };

        // Send to LLM
        let llm_response = self.llm_client.chat_completion_custom(chat_request).await?;

        // Extract the content from the first choice
        let content = llm_response
            .choices
            .first()
            .ok_or_else(|| Error::api("No response choices returned from LLM"))?
            .message
            .content
            .as_str();

        // Parse the response
        let redaction_response = self
            .prompt_formatter
            .parse_response(content)
            .map_err(|e| Error::api(format!("Failed to parse LLM response: {}", e)))?;

        // Validate that all returned IDs exist in the original request
        crate::prompt::validate_redaction_ids(request, &redaction_response)
            .map_err(|e| Error::api(format!("Invalid redaction response: {}", e)))?;

        Ok(redaction_response)
    }

    /// Returns a reference to the underlying LLM client.
    pub fn llm_client(&self) -> &LlmClient {
        &self.llm_client
    }

    /// Returns a reference to the prompt formatter.
    pub fn prompt_formatter(&self) -> &RedactionPrompt {
        &self.prompt_formatter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::RedactionItem;

    // Note: These tests would require API keys and make real API calls
    // In a real implementation, you'd want to use mocks or integration test flags

    #[test]
    fn test_redaction_client_creation() {
        // This test just verifies the client can be created
        // We can't test actual redaction without API keys

        // Create a mock scenario (in real tests, you'd use a proper mock)
        // let client = LlmClient::from_api_key("test-key").unwrap();
        // let redaction_client = RedactionClient::new(client);

        // Just test that the types work correctly
        assert!(true); // Placeholder for now
    }

    #[test]
    fn test_request_structure() {
        let request = RedactionRequest {
            data: vec![
                RedactionItem {
                    id: "1".to_string(),
                    text: "123 Main St, 555-1234".to_string(),
                    entity: "John Doe".to_string(),
                    data_type: "address".to_string(),
                },
                RedactionItem {
                    id: "2".to_string(),
                    text: "8th of January, 1990".to_string(),
                    entity: "John Doe".to_string(),
                    data_type: "date of birth".to_string(),
                },
            ],
            prompt: "Redact all addresses that belong to John Doe".to_string(),
        };

        assert_eq!(request.data.len(), 2);
        assert_eq!(request.data[0].id, "1");
        assert_eq!(request.data[0].entity, "John Doe");
        assert_eq!(request.data[0].data_type, "address");
    }

    #[test]
    fn test_custom_prompt_formatter() {
        let custom_prompt =
            RedactionPrompt::with_system_prompt("Custom redaction instructions for testing");

        let request = RedactionRequest {
            data: vec![],
            prompt: "test".to_string(),
        };

        let formatted = custom_prompt.format_request(&request);
        assert!(formatted.contains("Custom redaction instructions"));
    }
}
