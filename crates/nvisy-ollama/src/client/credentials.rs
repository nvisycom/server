//! Authentication credentials for Ollama
//!
//! This module provides authentication credential types and constructors for the Ollama client.

/// Authentication credentials for the Ollama service
///
/// Supports different authentication methods including API keys,
/// bearer tokens, and basic authentication. Many local Ollama instances
/// don't require authentication.
#[derive(Debug, Clone)]
pub enum OllamaCredentials {
    /// API key authentication (used with some Ollama deployments)
    ApiKey(String),
    /// Bearer token authentication
    BearerToken(String),
    /// Basic authentication with username and password
    Basic { username: String, password: String },
    /// No authentication (common for local Ollama instances)
    None,
}

impl OllamaCredentials {
    /// Create API key credentials
    pub fn api_key(key: impl Into<String>) -> Self {
        Self::ApiKey(key.into())
    }

    /// Create bearer token credentials
    pub fn bearer_token(token: impl Into<String>) -> Self {
        Self::BearerToken(token.into())
    }

    /// Create basic authentication credentials
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Create credentials with no authentication (default for local Ollama)
    pub fn none() -> Self {
        Self::None
    }
}

impl Default for OllamaCredentials {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials() {
        let api_key = OllamaCredentials::api_key("test-key");
        let bearer = OllamaCredentials::bearer_token("test-token");
        let basic = OllamaCredentials::basic("user", "pass");
        let none = OllamaCredentials::none();

        match api_key {
            OllamaCredentials::ApiKey(key) => assert_eq!(key, "test-key"),
            _ => panic!("Expected API key credentials"),
        }

        match bearer {
            OllamaCredentials::BearerToken(token) => assert_eq!(token, "test-token"),
            _ => panic!("Expected bearer token credentials"),
        }

        match basic {
            OllamaCredentials::Basic { username, password } => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Expected basic credentials"),
        }

        match none {
            OllamaCredentials::None => {}
            _ => panic!("Expected no credentials"),
        }
    }

    #[test]
    fn test_default_credentials() {
        let default = OllamaCredentials::default();
        match default {
            OllamaCredentials::None => {}
            _ => panic!("Expected default credentials to be None"),
        }
    }
}
