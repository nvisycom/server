//! Redaction prompt creation functions.

use super::redaction_request::{RedactionItem, RedactionRequest};

/// Creates the system prompt for redaction tasks.
///
/// This returns the default system instructions that guide the LLM
/// on how to perform redaction analysis, including the available category list.
///
/// # Returns
///
/// A string containing the system prompt with category definitions
///
/// # Example
///
/// ```rust
/// use nvisy_portkey::completion::redaction_prompts::create_system_prompt;
///
/// let system_prompt = create_system_prompt();
/// println!("{}", system_prompt);
/// ```
pub fn create_system_prompt() -> String {
    use strum::IntoEnumIterator;

    use super::redaction_categories::RedactionCategory;

    let categories: Vec<String> = RedactionCategory::iter()
        .map(|c| format!("  - {}", c))
        .collect();
    let categories_list = categories.join("\n");

    format!(
        r#"You are a data privacy assistant that helps identify which data items should be redacted based on specific criteria.

You will receive a list of data items, each with:
- An ID (UUID)
- Text content that may contain sensitive data
- Optional entity name (e.g., person or organization name)
- Optional specific categories to look for

## Available Categories

When categorizing data, use EXACTLY ONE of these predefined categories:

{}

IMPORTANT: Use the exact category name as shown above (e.g., "Email Addresses", "Full SSNs", "Credit Cards").

Your task is to analyze the data and return a JSON object with:
1. "entities": Array of entities found, each with "name" and optional "category"
2. "data": Array of items to redact, each with:
   - "id": The original item UUID
   - "data": The specific data to redact
   - "category": One of the predefined categories listed above
   - "entity": Optional entity name this data belongs to

Guidelines:
- Only redact items that clearly match the specified criteria
- Be precise - don't redact items unless they explicitly match the request
- Use the EXACT category names from the list above
- Consider the entity and text content when making decisions
- If categories are specified in the request, prioritize those
- Return valid JSON only, no other text

Example response:
{{
  "entities": [
    {{"name": "John Smith", "category": "Full Names"}}
  ],
  "data": [
    {{
      "id": "123e4567-e89b-12d3-a456-426614174000",
      "data": "123 Main St",
      "category": "Addresses",
      "entity": "John Smith"
    }}
  ]
}}"#,
        categories_list
    )
}

/// Creates the user prompt for a redaction request.
///
/// This formats the redaction request into a prompt that can be sent to the LLM,
/// including the data items, categories, and user's redaction criteria.
///
/// # Arguments
///
/// * `request` - The complete redaction request
///
/// # Returns
///
/// A formatted string containing the user prompt with data and instructions
///
/// # Example
///
/// ```rust
/// use nvisy_portkey::completion::{RedactionRequest, RedactionItem, redaction_prompts::create_user_prompt};
///
/// let request = RedactionRequest::new(
///     vec![RedactionItem::new("123 Main St, 555-1234").with_entity("John Doe")],
///     "Redact all addresses that belong to John Doe"
/// );
/// let user_prompt = create_user_prompt(&request);
/// ```
pub fn create_user_prompt(request: &RedactionRequest) -> String {
    let data_json =
        serde_json::to_string_pretty(&request.data).unwrap_or_else(|_| "[]".to_string());

    let categories_section = if let Some(categories) = &request.categories {
        let categories_list: Vec<String> =
            categories.iter().map(|c| format!("  - {}", c)).collect();
        format!(
            "\n## Priority Categories\n\nFocus on finding data in these categories:\n\n{}\n",
            categories_list.join("\n")
        )
    } else {
        String::new()
    };

    format!(
        r#"## Data Items
```json
{}
```
{}
## Redaction Request
{}

## Instructions
Analyze the data items above and return a JSON object with:
1. "entities" array containing all entities found (with categories from the predefined list)
2. "data" array containing items to redact according to the request (with categories from the predefined list)

Remember to use EXACT category names from the system prompt.
Return only the JSON object, no other text."#,
        data_json, categories_section, request.prompt
    )
}

/// Creates the user prompt (legacy version for backward compatibility).
///
/// This is a convenience wrapper that creates a RedactionRequest internally.
///
/// # Arguments
///
/// * `data` - List of data items to analyze
/// * `prompt` - User's redaction criteria
///
/// # Returns
///
/// A formatted string containing the user prompt with data and instructions
pub fn create_user_prompt_legacy(data: &[RedactionItem], prompt: &str) -> String {
    let request = RedactionRequest::new(data.to_vec(), prompt);
    create_user_prompt(&request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_system_prompt() {
        let prompt = create_system_prompt();
        assert!(!prompt.is_empty());
        assert!(prompt.contains("data privacy"));
        assert!(prompt.contains("entities"));
        assert!(prompt.contains("Available Categories"));
        assert!(prompt.contains("Email Addresses"));
        assert!(prompt.contains("Full Names"));
    }

    #[test]
    fn test_create_user_prompt() {
        let request = RedactionRequest::new(
            vec![RedactionItem::new("123 Main St").with_entity("John Doe")],
            "Redact all addresses",
        );
        let user_prompt = create_user_prompt(&request);

        assert!(user_prompt.contains("123 Main St"));
        assert!(user_prompt.contains("Redact all addresses"));
        assert!(user_prompt.contains("JSON object"));
    }

    #[test]
    fn test_user_prompt_with_empty_data() {
        let request = RedactionRequest::new(vec![], "Test prompt");
        let user_prompt = create_user_prompt(&request);
        assert!(user_prompt.contains("Test prompt"));
        assert!(user_prompt.contains("[]"));
    }

    #[test]
    fn test_user_prompt_with_categories() {
        use super::super::redaction_categories::RedactionCategory;

        let request = RedactionRequest::new(
            vec![RedactionItem::new("john@example.com")],
            "Find sensitive data",
        )
        .with_categories(vec![RedactionCategory::EmailAddresses]);

        let user_prompt = create_user_prompt(&request);
        assert!(user_prompt.contains("Priority Categories"));
        assert!(user_prompt.contains("Email Addresses"));
    }

    #[test]
    fn test_create_user_prompt_legacy() {
        let items = vec![RedactionItem::new("123 Main St")];
        let user_prompt = create_user_prompt_legacy(&items, "Test prompt");
        assert!(user_prompt.contains("123 Main St"));
        assert!(user_prompt.contains("Test prompt"));
    }
}
