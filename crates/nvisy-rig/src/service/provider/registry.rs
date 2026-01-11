//! Provider registry for managing multiple AI providers.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::config::ProviderConfig;
use crate::{Error, Result};

/// Reference to a specific model in format "provider_id/model_name".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelRef {
    /// Provider ID.
    pub provider_id: String,
    /// Model name.
    pub model: String,
}

impl ModelRef {
    /// Creates a new model reference.
    pub fn new(provider_id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            model: model.into(),
        }
    }
}

impl FromStr for ModelRef {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let (provider_id, model) = s.split_once('/').ok_or_else(|| {
            Error::config(format!(
                "invalid model reference '{}': expected 'provider/model'",
                s
            ))
        })?;

        Ok(Self::new(provider_id, model))
    }
}

impl std::fmt::Display for ModelRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.provider_id, self.model)
    }
}

/// Default models for different tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultModels {
    /// Default model for embeddings.
    pub embedding: ModelRef,
    /// Default model for completions/chat.
    pub completion: ModelRef,
    /// Default model for vision tasks.
    pub vision: ModelRef,
}

/// Registry of configured AI providers.
///
/// Allows selecting providers per-request from a set of globally configured providers.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<ProviderConfig>>,
    defaults: DefaultModels,
}

impl ProviderRegistry {
    /// Creates a new provider registry.
    pub fn new(providers: Vec<ProviderConfig>, defaults: DefaultModels) -> Result<Self> {
        let mut provider_map = HashMap::new();

        for config in providers {
            if provider_map.contains_key(&config.id) {
                return Err(Error::config(format!(
                    "duplicate provider id: {}",
                    config.id
                )));
            }
            provider_map.insert(config.id.clone(), Arc::new(config));
        }

        // Validate defaults exist
        if !provider_map.contains_key(&defaults.embedding.provider_id) {
            return Err(Error::config(format!(
                "default embedding provider not found: {}",
                defaults.embedding.provider_id
            )));
        }
        if !provider_map.contains_key(&defaults.completion.provider_id) {
            return Err(Error::config(format!(
                "default completion provider not found: {}",
                defaults.completion.provider_id
            )));
        }
        if !provider_map.contains_key(&defaults.vision.provider_id) {
            return Err(Error::config(format!(
                "default vision provider not found: {}",
                defaults.vision.provider_id
            )));
        }

        Ok(Self {
            providers: provider_map,
            defaults,
        })
    }

    /// Gets a provider by ID.
    pub fn get(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.get(id).map(|p| p.as_ref())
    }

    /// Gets the provider for a model reference, falling back to defaults.
    pub fn resolve_embedding(
        &self,
        model_ref: Option<&ModelRef>,
    ) -> Result<(&ProviderConfig, String)> {
        let model_ref = model_ref.unwrap_or(&self.defaults.embedding);
        self.resolve(model_ref)
    }

    /// Gets the provider for a completion model reference, falling back to defaults.
    pub fn resolve_completion(
        &self,
        model_ref: Option<&ModelRef>,
    ) -> Result<(&ProviderConfig, String)> {
        let model_ref = model_ref.unwrap_or(&self.defaults.completion);
        self.resolve(model_ref)
    }

    /// Gets the provider for a vision model reference, falling back to defaults.
    pub fn resolve_vision(
        &self,
        model_ref: Option<&ModelRef>,
    ) -> Result<(&ProviderConfig, String)> {
        let model_ref = model_ref.unwrap_or(&self.defaults.vision);
        self.resolve(model_ref)
    }

    /// Resolves a model reference to provider config and model name.
    fn resolve(&self, model_ref: &ModelRef) -> Result<(&ProviderConfig, String)> {
        let provider = self.providers.get(&model_ref.provider_id).ok_or_else(|| {
            Error::config(format!("provider not found: {}", model_ref.provider_id))
        })?;

        Ok((provider.as_ref(), model_ref.model.clone()))
    }

    /// Returns all registered provider IDs.
    pub fn provider_ids(&self) -> impl Iterator<Item = &str> {
        self.providers.keys().map(|s| s.as_str())
    }

    /// Returns the default models.
    pub fn defaults(&self) -> &DefaultModels {
        &self.defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_ref_parse() {
        let model_ref: ModelRef = "openai/gpt-4o"
            .parse()
            .expect("valid model ref format should parse");
        assert_eq!(model_ref.provider_id, "openai");
        assert_eq!(model_ref.model, "gpt-4o");
    }

    #[test]
    fn test_model_ref_display() {
        let model_ref = ModelRef::new("anthropic", "claude-sonnet-4-20250514");
        assert_eq!(model_ref.to_string(), "anthropic/claude-sonnet-4-20250514");
    }

    #[test]
    fn test_model_ref_invalid() {
        let result: Result<ModelRef> = "invalid".parse();
        assert!(result.is_err());
    }
}
