//! Authentication secret keys management for JWT session handling.
//!
//! This module provides functionality for loading and managing cryptographic keys
//! used in JWT-based session authentication.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(any(test, feature = "config"))]
use clap::Args;
use jsonwebtoken::{DecodingKey, EncodingKey};
use serde::{Deserialize, Serialize};

use crate::utility::tracing_targets::SESSION_KEYS as TRACING_TARGET;
use crate::{Error, Result};

/// Authentication key file paths configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "config"), derive(Args))]
pub struct SessionKeysConfig {
    /// File path to the JWT decoding (public) key used for sessions.
    #[cfg_attr(
        any(test, feature = "config"),
        arg(long, env = "AUTH_PUBLIC_PEM_FILEPATH", default_value = "./public.pem")
    )]
    #[serde(default = "SessionKeysConfig::default_decoding_key")]
    pub decoding_key: PathBuf,

    /// File path to the JWT encoding (private) key used for sessions.
    #[cfg_attr(
        any(test, feature = "config"),
        arg(
            long,
            env = "AUTH_PRIVATE_PEM_FILEPATH",
            default_value = "./private.pem"
        )
    )]
    #[serde(default = "SessionKeysConfig::default_encoding_key")]
    pub encoding_key: PathBuf,
}

impl SessionKeysConfig {
    fn default_decoding_key() -> PathBuf {
        "./public.pem".into()
    }

    fn default_encoding_key() -> PathBuf {
        "./private.pem".into()
    }
}

/// Secret keys used for JWT session authentication.
///
/// This struct provides thread-safe access to cryptographic keys used for
/// encoding and decoding JWT tokens in session management.
#[derive(Clone)]
pub struct SessionKeys {
    inner: Arc<AuthKeysInner>,
}

/// Internal container for the actual key data.
struct AuthKeysInner {
    decoding_key: DecodingKey,
    encoding_key: EncodingKey,
    config: SessionKeysConfig,
}

impl SessionKeys {
    /// Creates a new `AuthKeys` instance from the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration containing paths to the key files
    ///
    /// # Returns
    ///
    /// Returns a result containing the initialized keys or an error.
    pub async fn from_config(config: &SessionKeysConfig) -> Result<Self> {
        // Validate configuration before attempting to load keys
        Self::validate_config(config)?;

        tracing::debug!(
            target: TRACING_TARGET,
            decoding_key_path = %config.decoding_key.display(),
            encoding_key_path = %config.encoding_key.display(),
            "Loading authentication secret keys",
        );

        // Load and parse decoding key
        let decoding_key = Self::load_decoding_key(&config.decoding_key).await?;

        // Load and parse encoding key
        let encoding_key = Self::load_encoding_key(&config.encoding_key).await?;

        tracing::info!(
            target: TRACING_TARGET,
            "Authentication keys loaded",
        );

        let inner = Arc::new(AuthKeysInner {
            decoding_key,
            encoding_key,
            config: config.clone(),
        });

        Ok(Self { inner })
    }

    /// Creates a new `AuthKeys` instance from file paths.
    ///
    /// This is a convenience method that creates a configuration and loads the keys.
    ///
    /// # Arguments
    ///
    /// * `decoding_pem_key` - Path to the decoding key PEM file
    /// * `encoding_pem_key` - Path to the encoding key PEM file
    ///
    /// # Returns
    ///
    /// Returns a result containing the initialized keys or an error.
    pub async fn new(
        decoding_pem_key: impl AsRef<Path>,
        encoding_pem_key: impl AsRef<Path>,
    ) -> Result<Self> {
        let config = SessionKeysConfig {
            decoding_key: decoding_pem_key.as_ref().to_path_buf(),
            encoding_key: encoding_pem_key.as_ref().to_path_buf(),
        };
        Self::from_config(&config).await
    }

    /// Returns a reference to the decoding key.
    ///
    /// This key is used to verify JWT tokens.
    #[inline]
    pub fn decoding_key(&self) -> &DecodingKey {
        &self.inner.decoding_key
    }

    /// Returns a reference to the encoding key.
    ///
    /// This key is used to sign JWT tokens.
    #[inline]
    pub fn encoding_key(&self) -> &EncodingKey {
        &self.inner.encoding_key
    }

    /// Returns a reference to the configuration used to create this instance.
    #[inline]
    pub fn config(&self) -> &SessionKeysConfig {
        &self.inner.config
    }

    /// Validates that the loaded keys are functional for JWT operations.
    ///
    /// This method performs a round-trip test by creating and verifying a test JWT token.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if keys are valid, or an error if validation fails.
    pub fn validate_keys(&self) -> Result<()> {
        use jsonwebtoken::{Algorithm, Header, Validation, decode, encode};
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct TestClaims {
            sub: String,
            exp: usize,
        }

        let claims = TestClaims {
            sub: "test".to_string(),
            exp: (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 300) as usize, // 5 minutes from now
        };

        // Try to encode with the encoding key
        let header = Header::new(Algorithm::EdDSA);
        let token = encode(&header, &claims, self.encoding_key()).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %e,
                "key validation failed during encoding",
            );

            Error::auth("key validation encoding failed").with_source(e)
        })?;

        // Try to decode with the decoding key
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = true;

        decode::<TestClaims>(&token, self.decoding_key(), &validation).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %e,
                "key validation failed during decoding",
            );
            Error::auth("key validation decoding failed").with_source(e)
        })?;

        tracing::debug!(
            target: TRACING_TARGET,
            "key validation successful",
        );

        Ok(())
    }

    /// Validates that both key files exist and are readable.
    fn validate_config(config: &SessionKeysConfig) -> Result<()> {
        if !config.decoding_key.exists() {
            return Err(Error::config("Decoding key file does not exist"));
        }

        if !config.encoding_key.exists() {
            return Err(Error::config("Encoding key file does not exist"));
        }

        if !config.decoding_key.is_file() {
            return Err(Error::config("Decoding key path is not a file"));
        }

        if !config.encoding_key.is_file() {
            return Err(Error::config("Encoding key path is not a file"));
        }

        Ok(())
    }

    /// Loads and parses the decoding key from the configured path.
    async fn load_decoding_key(path: &Path) -> Result<DecodingKey> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path.display(),
            "loading decoding key from file",
        );

        let pem_data = tokio::fs::read(path).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                path = %path.display(),
                error = %e,
                "failed to read decoding key file",
            );
            Error::file_system("failed to read decoding key file").with_source(e)
        })?;

        let key = DecodingKey::from_ed_pem(&pem_data).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                path = %path.display(),
                error = %e,
                "failed to parse decoding key PEM data",
            );
            Error::auth("invalid decoding key PEM format").with_source(e)
        })?;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path.display(),
            key_size_bytes = pem_data.len(),
            "decoding key loaded successfully",
        );

        Ok(key)
    }

    /// Loads and parses the encoding key from the configured path.
    async fn load_encoding_key(path: &Path) -> Result<EncodingKey> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path.display(),
            "loading encoding key from file",
        );

        let pem_data = tokio::fs::read(path).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                path = %path.display(),
                error = %e,
                "failed to read encoding key file",
            );

            Error::file_system("failed to read encoding key file").with_source(e)
        })?;

        let key = EncodingKey::from_ed_pem(&pem_data).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                path = %path.display(),
                error = %e,
                "failed to parse encoding key PEM data",
            );

            Error::auth("invalid encoding key PEM format").with_source(e)
        })?;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path.display(),
            key_size_bytes = pem_data.len(),
            "encoding key loaded successfully",
        );

        Ok(key)
    }
}

impl fmt::Debug for SessionKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthKeys")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    const TEST_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIDQtFc/jcCECuwR6cQqh9Xy3y8pcryWDn/HVN5fPSwm+
-----END PRIVATE KEY-----"#;

    const TEST_PUBLIC_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAMveirBCUUpVI8TCv4W5jAZqtkEzfA7eIvozsugFbvDU=
-----END PUBLIC KEY-----"#;

    #[tokio::test]
    async fn load_valid_keys() {
        let temp_dir = TempDir::new().unwrap();
        let pub_path = temp_dir.path().join("public.pem");
        let priv_path = temp_dir.path().join("private.pem");

        fs::write(&pub_path, TEST_PUBLIC_KEY).unwrap();
        fs::write(&priv_path, TEST_PRIVATE_KEY).unwrap();

        let keys = SessionKeys::new(&pub_path, &priv_path).await.unwrap();
        let result = keys.validate_keys();
        assert!(result.is_ok(), "validate_keys failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn reject_invalid_key_format() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_path = temp_dir.path().join("invalid.pem");
        let priv_path = temp_dir.path().join("private.pem");

        fs::write(&invalid_path, "invalid pem").unwrap();
        fs::write(&priv_path, TEST_PRIVATE_KEY).unwrap();

        assert!(SessionKeys::new(&invalid_path, &priv_path).await.is_err());
    }

    #[tokio::test]
    async fn reject_missing_files() {
        let temp_dir = TempDir::new().unwrap();
        let pub_path = temp_dir.path().join("nonexistent_public.pem");
        let priv_path = temp_dir.path().join("nonexistent_private.pem");
        
        assert!(SessionKeys::new(&pub_path, &priv_path).await.is_err());
    }
}
