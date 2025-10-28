//! Redaction response types.

use std::str::FromStr;

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::categories::RedactionCategory;
use crate::REDACTION_TARGET;
use crate::error::Error;

/// An entity identified in the redaction analysis.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSchema,
    PartialEq,
    Eq,
    Builder
)]
#[builder(pattern = "owned", setter(into, strip_option, prefix = "with"))]
pub struct Entity {
    /// Name of the entity (e.g., "John Smith")
    pub name: String,

    /// Category of the entity
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub category: Option<RedactionCategory>,
}

impl Entity {
    /// Creates a new entity.
    ///
    /// # Arguments
    ///
    /// * `name` - The entity name
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::Entity;
    ///
    /// let entity = Entity::new("John Smith");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            category: None,
        }
    }

    /// Sets the category for this entity.
    ///
    /// # Arguments
    ///
    /// * `category` - The entity category
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::{Entity, RedactionCategory};
    ///
    /// let entity = Entity::new("John Smith")
    ///     .with_category(RedactionCategory::FullNames);
    /// ```
    pub fn with_category(mut self, category: RedactionCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Returns a builder for creating an Entity.
    pub fn builder() -> EntityBuilder {
        EntityBuilder::default()
    }
}

/// A piece of data that should be redacted.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into, strip_option, prefix = "with"))]
#[schemars(bound = "Uuid: JsonSchema")]
pub struct RedactedData {
    /// The original item ID from the request
    #[schemars(with = "String")]
    pub id: Uuid,

    /// The data content to redact (e.g., "Main St 12")
    pub data: String,

    /// Category of the data (e.g., "Addresses", "Phone Numbers")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub category: Option<RedactionCategory>,

    /// Optional entity this data belongs to (e.g., "John Smith")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub entity: Option<String>,
}

impl RedactedData {
    /// Creates a new redacted data entry.
    ///
    /// # Arguments
    ///
    /// * `id` - The original item ID
    /// * `data` - The data content to redact
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactedData;
    /// use uuid::Uuid;
    ///
    /// let id = Uuid::new_v4();
    /// let data = RedactedData::new(id, "123 Main St");
    /// ```
    pub fn new(id: Uuid, data: impl Into<String>) -> Self {
        Self {
            id,
            data: data.into(),
            category: None,
            entity: None,
        }
    }

    /// Sets the category for this data.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::{RedactedData, RedactionCategory};
    /// use uuid::Uuid;
    ///
    /// let id = Uuid::new_v4();
    /// let data = RedactedData::new(id, "123 Main St")
    ///     .with_category(RedactionCategory::Addresses);
    /// ```
    pub fn with_category(mut self, category: RedactionCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Sets the entity for this data.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactedData;
    /// use uuid::Uuid;
    ///
    /// let id = Uuid::new_v4();
    /// let data = RedactedData::new(id, "123 Main St")
    ///     .with_entity("John Doe");
    /// ```
    pub fn with_entity(mut self, entity: impl Into<String>) -> Self {
        self.entity = Some(entity.into());
        self
    }

    /// Returns a builder for creating a RedactedData.
    pub fn builder() -> RedactedDataBuilder {
        RedactedDataBuilder::default()
    }
}

/// Internal representation for deserializing with string categories
#[derive(Debug, Deserialize)]
struct EntityWithStringCategory {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
}

/// Internal representation for deserializing with string categories
#[derive(Debug, Deserialize)]
struct RedactedDataWithStringCategory {
    id: Uuid,
    data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity: Option<String>,
}

/// Internal representation for initial deserialization
#[derive(Debug, Deserialize)]
struct RedactionResponseRaw {
    #[serde(default)]
    entities: Vec<EntityWithStringCategory>,
    #[serde(default)]
    data: Vec<RedactedDataWithStringCategory>,
}

/// Response from the LLM containing redaction analysis results.
#[derive(
    Debug,
    Default,
    Clone,
    Serialize,
    Deserialize,
    JsonSchema,
    PartialEq,
    Builder
)]
#[builder(pattern = "owned", setter(into, prefix = "with"))]
pub struct RedactionResponse {
    /// List of entities identified in the data
    #[builder(default)]
    pub entities: Vec<Entity>,

    /// List of data items that should be redacted
    #[builder(default)]
    pub data: Vec<RedactedData>,
}

impl RedactionResponse {
    /// Creates a new empty redaction response.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a JSON response from the LLM into a RedactionResponse.
    ///
    /// This method:
    /// - Handles various response formats (pure JSON, markdown-wrapped, etc.)
    /// - Gracefully handles invalid categories by logging and skipping them
    /// - Does not fail the entire parse if individual categories are invalid
    ///
    /// # Arguments
    ///
    /// * `response` - The raw response string from the LLM
    ///
    /// # Returns
    ///
    /// A parsed `RedactionResponse` with valid categories
    ///
    /// # Errors
    ///
    /// Returns an error only if the JSON structure itself is invalid,
    /// not if individual categories cannot be parsed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionResponse;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let json = r#"{"entities": [], "data": []}"#;
    /// let response = RedactionResponse::from_llm_response(json)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_llm_response(response: &str) -> Result<Self, Error> {
        // Clean the response - remove markdown code blocks and extra whitespace
        let mut response = response.trim().strip_prefix("```json").unwrap_or(response);
        response = response.strip_prefix("```").unwrap_or(response);
        let cleaned = response.strip_suffix("```").unwrap_or(response).trim();

        // Parse JSON with string categories first
        let raw = Self::parse_json(cleaned)?;

        // Convert to final types with graceful category parsing
        Ok(Self {
            entities: Self::parse_entities(raw.entities),
            data: Self::parse_redacted_data(raw.data),
        })
    }

    /// Parses the JSON structure (internal helper).
    fn parse_json(cleaned: &str) -> Result<RedactionResponseRaw, Error> {
        match serde_json::from_str::<RedactionResponseRaw>(cleaned) {
            Ok(response) => Ok(response),
            Err(_) => {
                // Fallback: try to extract JSON object from text
                if let Some(start) = cleaned.find('{') {
                    if let Some(end) = cleaned.rfind('}') {
                        let json_part = &cleaned[start..=end];
                        serde_json::from_str::<RedactionResponseRaw>(json_part).map_err(|e| {
                            Error::Api {
                                message: format!("Failed to parse LLM response: {}", e),
                                status_code: None,
                                error_code: Some("parse_error".to_string()),
                            }
                        })
                    } else {
                        Err(Error::Api {
                            message: "No JSON object found in response".to_string(),
                            status_code: None,
                            error_code: Some("no_json".to_string()),
                        })
                    }
                } else {
                    Err(Error::Api {
                        message: "No JSON object found in response".to_string(),
                        status_code: None,
                        error_code: Some("no_json".to_string()),
                    })
                }
            }
        }
    }

    /// Parses entities with graceful category handling.
    fn parse_entities(raw_entities: Vec<EntityWithStringCategory>) -> Vec<Entity> {
        raw_entities
            .into_iter()
            .map(|raw| {
                let category =
                    raw.category
                        .and_then(|cat_str| match RedactionCategory::from_str(&cat_str) {
                            Ok(category) => Some(category),
                            Err(e) => {
                                tracing::warn!(
                                    target: REDACTION_TARGET,
                                    category = %cat_str,
                                    error = %e,
                                    "Failed to parse entity category, skipping"
                                );
                                None
                            }
                        });

                Entity {
                    name: raw.name,
                    category,
                }
            })
            .collect()
    }

    /// Parses redacted data with graceful category handling.
    fn parse_redacted_data(raw_data: Vec<RedactedDataWithStringCategory>) -> Vec<RedactedData> {
        raw_data
            .into_iter()
            .filter_map(|raw| {
                // Skip entries with invalid UUIDs
                let category =
                    raw.category
                        .and_then(|cat_str| match RedactionCategory::from_str(&cat_str) {
                            Ok(category) => Some(category),
                            Err(e) => {
                                tracing::warn!(
                                    target: REDACTION_TARGET,
                                    id = %raw.id,
                                    category = %cat_str,
                                    error = %e,
                                    "Failed to parse data category, skipping category"
                                );
                                None
                            }
                        });

                Some(RedactedData {
                    id: raw.id,
                    data: raw.data,
                    category,
                    entity: raw.entity,
                })
            })
            .collect()
    }

    /// Returns a builder for creating a RedactionResponse.
    pub fn builder() -> RedactionResponseBuilder {
        RedactionResponseBuilder::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_new() {
        let entity = Entity::new("John Smith");
        assert_eq!(entity.name, "John Smith");
        assert!(entity.category.is_none());
    }

    #[test]
    fn test_entity_with_category() {
        let entity = Entity::new("John Smith").with_category(RedactionCategory::FullNames);
        assert_eq!(entity.name, "John Smith");
        assert_eq!(entity.category, Some(RedactionCategory::FullNames));
    }

    #[test]
    fn test_redacted_data_new() {
        let id = Uuid::new_v4();
        let data = RedactedData::new(id, "123 Main St");

        assert_eq!(data.id, id);
        assert_eq!(data.data, "123 Main St");
        assert!(data.category.is_none());
        assert!(data.entity.is_none());
    }

    #[test]
    fn test_redacted_data_with_category() {
        let id = Uuid::new_v4();
        let data = RedactedData::new(id, "123 Main St").with_category(RedactionCategory::Addresses);

        assert_eq!(data.category, Some(RedactionCategory::Addresses));
    }

    #[test]
    fn test_redaction_response_from_llm_response() -> crate::Result<()> {
        let json =
            r#"{"entities": [{"name": "John Smith", "category": "Full Names"}], "data": []}"#;
        let response = RedactionResponse::from_llm_response(json)?;

        assert_eq!(response.entities.len(), 1);
        assert_eq!(response.entities[0].name, "John Smith");
        assert_eq!(
            response.entities[0].category,
            Some(RedactionCategory::FullNames)
        );
        assert_eq!(response.data.len(), 0);
        Ok(())
    }

    #[test]
    fn test_redaction_response_with_invalid_category() -> crate::Result<()> {
        // Should not fail even with invalid category
        let json = r#"{"entities": [{"name": "John", "category": "InvalidCategory"}], "data": []}"#;
        let response = RedactionResponse::from_llm_response(json)?;

        assert_eq!(response.entities.len(), 1);
        assert_eq!(response.entities[0].name, "John");
        // Category should be None because it couldn't be parsed
        assert_eq!(response.entities[0].category, None);
        Ok(())
    }

    #[test]
    fn test_redaction_response_with_markdown() -> crate::Result<()> {
        let json = "```json\n{\"entities\": [], \"data\": []}\n```";
        let response = RedactionResponse::from_llm_response(json)?;

        assert_eq!(response.entities.len(), 0);
        assert_eq!(response.data.len(), 0);
        Ok(())
    }

    #[test]
    fn test_redacted_data_with_valid_category() -> crate::Result<()> {
        let id = Uuid::new_v4();
        let json = format!(
            r#"{{"entities": [], "data": [{{"id": "{}", "data": "test@example.com", "category": "Email Addresses"}}]}}"#,
            id
        );
        let response = RedactionResponse::from_llm_response(&json)?;

        assert_eq!(response.data.len(), 1);
        assert_eq!(
            response.data[0].category,
            Some(RedactionCategory::EmailAddresses)
        );
        Ok(())
    }
}
