//! NATS authentication credentials.

use serde::{Deserialize, Serialize};

/// NATS authentication credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NatsCredentials {
    /// Username and password authentication
    UserPassword {
        /// Username for authentication
        user: String,
        /// Password for authentication
        pass: String,
    },
    /// JWT token authentication
    Token {
        /// JWT token string
        token: String,
    },
    /// Credentials file path (contains JWT and NKey)
    CredsFile {
        /// Path to the credentials file
        path: String,
    },
    /// NKey seed for cryptographic authentication
    NKey {
        /// NKey seed string
        seed: String,
    },
}

impl NatsCredentials {
    /// Create user/password credentials.
    pub fn user_password(user: impl Into<String>, pass: impl Into<String>) -> Self {
        Self::UserPassword {
            user: user.into(),
            pass: pass.into(),
        }
    }

    /// Create token-based credentials.
    pub fn token(token: impl Into<String>) -> Self {
        Self::Token {
            token: token.into(),
        }
    }

    /// Create credentials from a file path.
    pub fn creds_file(path: impl Into<String>) -> Self {
        Self::CredsFile { path: path.into() }
    }

    /// Create NKey-based credentials.
    pub fn nkey(seed: impl Into<String>) -> Self {
        Self::NKey { seed: seed.into() }
    }
}
