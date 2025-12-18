//! Authentication credentials
//!
//! This module provides authentication credential types and constructors for the OCR client.

/// Authentication credentials for the OCR service
///
/// Supports different authentication methods including API keys,
/// bearer tokens, and basic authentication.
#[derive(Debug, Clone)]
pub enum OlemCredentials {
    /// API key authentication
    ApiKey(String),
    /// Bearer token authentication
    BearerToken(String),
    /// Basic authentication with username and password
    Basic { username: String, password: String },
    /// No authentication (for testing/development)
    None,
}

impl OlemCredentials {
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

    /// Create credentials with no authentication
    pub fn none() -> Self {
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials() {
        let api_key = OlemCredentials::api_key("test-key");
        let bearer = OlemCredentials::bearer_token("test-token");
        let basic = OlemCredentials::basic("user", "pass");
        let none = OlemCredentials::none();

        match api_key {
            OlemCredentials::ApiKey(key) => assert_eq!(key, "test-key"),
            _ => panic!("Expected API key credentials"),
        }

        match bearer {
            OlemCredentials::BearerToken(token) => assert_eq!(token, "test-token"),
            _ => panic!("Expected bearer token credentials"),
        }

        match basic {
            OlemCredentials::Basic { username, password } => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Expected basic credentials"),
        }

        match none {
            OlemCredentials::None => {}
            _ => panic!("Expected no credentials"),
        }
    }
}
