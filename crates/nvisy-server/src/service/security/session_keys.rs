//! Authentication secret keys management for JWT session handling.
//!
//! This module provides functionality for loading and managing cryptographic keys
//! used in JWT-based session authentication.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use jsonwebtoken::{DecodingKey, EncodingKey};

use crate::service::{Result, ServiceError};

/// Logging target for authentication key operations.
const TRACING_TARGET_AUTH_KEYS: &str = "nvisy_server::service::auth_keys";

/// Default name for the decoding key file.
const DECODING_KEY_FILE: &str = "public.pem";

/// Default name for the encoding key file.
const ENCODING_KEY_FILE: &str = "private.pem";

/// Configuration for authentication secret keys.
#[derive(Debug, Clone)]
pub struct AuthKeysConfig {
    /// Path to the PEM file containing the decoding key.
    pub decoding_key_path: PathBuf,
    /// Path to the PEM file containing the encoding key.
    pub encoding_key_path: PathBuf,
}

impl AuthKeysConfig {
    /// Creates a new configuration with the specified key file paths.
    pub fn new(decoding_key_path: impl AsRef<Path>, encoding_key_path: impl AsRef<Path>) -> Self {
        Self {
            decoding_key_path: decoding_key_path.as_ref().to_path_buf(),
            encoding_key_path: encoding_key_path.as_ref().to_path_buf(),
        }
    }

    /// Creates a new configuration using a base directory and default file names.
    pub fn from_directory(base_dir: impl AsRef<Path>) -> Self {
        let base_dir = base_dir.as_ref();
        Self {
            decoding_key_path: base_dir.join(DECODING_KEY_FILE),
            encoding_key_path: base_dir.join(ENCODING_KEY_FILE),
        }
    }

    /// Validates that both key files exist and are readable.
    pub fn validate(&self) -> Result<()> {
        if !self.decoding_key_path.exists() {
            return Err(ServiceError::config("Decoding key file does not exist"));
        }

        if !self.encoding_key_path.exists() {
            return Err(ServiceError::config("Encoding key file does not exist"));
        }

        // Check if files are readable
        if !self.decoding_key_path.is_file() {
            return Err(ServiceError::config("Decoding key path is not a file"));
        }

        if !self.encoding_key_path.is_file() {
            return Err(ServiceError::config("Encoding key path is not a file"));
        }

        Ok(())
    }

    /// Returns the decoding key path.
    #[inline]
    pub fn decoding_key_path(&self) -> &Path {
        &self.decoding_key_path
    }

    /// Returns the encoding key path.
    #[inline]
    pub fn encoding_key_path(&self) -> &Path {
        &self.encoding_key_path
    }
}

impl Default for AuthKeysConfig {
    fn default() -> Self {
        Self::from_directory(".")
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
    config: AuthKeysConfig,
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
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_server::service::auth::{AuthKeys, AuthSecretKeysConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let config = AuthSecretKeysConfig::new("decode.pem", "encode.pem");
    ///     let keys = AuthKeys::from_config(config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_config(config: AuthKeysConfig) -> Result<Self> {
        // Validate configuration before attempting to load keys
        config.validate()?;

        tracing::info!(
            target: TRACING_TARGET_AUTH_KEYS,
            decoding_key_path = %config.decoding_key_path().display(),
            encoding_key_path = %config.encoding_key_path().display(),
            "loading authentication secret keys",
        );

        // Load and parse decoding key
        let decoding_key = Self::load_decoding_key(&config).await?;

        // Load and parse encoding key
        let encoding_key = Self::load_encoding_key(&config).await?;

        tracing::info!(
            target: TRACING_TARGET_AUTH_KEYS,
            "authentication secret keys loaded successfully",
        );

        let inner = Arc::new(AuthKeysInner {
            decoding_key,
            encoding_key,
            config,
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
        let config = AuthKeysConfig::new(decoding_pem_key, encoding_pem_key);
        Self::from_config(config).await
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
    pub fn config(&self) -> &AuthKeysConfig {
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
                target: TRACING_TARGET_AUTH_KEYS,
                error = %e,
                "key validation failed during encoding",
            );

            ServiceError::auth("key validation encoding failed").with_source(e)
        })?;

        // Try to decode with the decoding key
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = true;

        decode::<TestClaims>(&token, self.decoding_key(), &validation).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_AUTH_KEYS,
                error = %e,
                "key validation failed during decoding",
            );
            ServiceError::auth("key validation decoding failed").with_source(e)
        })?;

        tracing::debug!(
            target: TRACING_TARGET_AUTH_KEYS,
            "key validation successful",
        );

        Ok(())
    }

    /// Loads and parses the decoding key from the configured path.
    async fn load_decoding_key(config: &AuthKeysConfig) -> Result<DecodingKey> {
        let path = config.decoding_key_path();

        tracing::debug!(
            target: TRACING_TARGET_AUTH_KEYS,
            path = %path.display(),
            "loading decoding key from file",
        );

        let pem_data = tokio::fs::read(path).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_AUTH_KEYS,
                path = %path.display(),
                error = %e,
                "failed to read decoding key file",
            );
            ServiceError::file_system("failed to read decoding key file").with_source(e)
        })?;

        let key = DecodingKey::from_ed_pem(&pem_data).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_AUTH_KEYS,
                path = %path.display(),
                error = %e,
                "failed to parse decoding key PEM data",
            );
            ServiceError::auth("invalid decoding key PEM format").with_source(e)
        })?;

        tracing::debug!(
            target: TRACING_TARGET_AUTH_KEYS,
            path = %path.display(),
            key_size_bytes = pem_data.len(),
            "decoding key loaded successfully",
        );

        Ok(key)
    }

    /// Loads and parses the encoding key from the configured path.
    async fn load_encoding_key(config: &AuthKeysConfig) -> Result<EncodingKey> {
        let path = config.encoding_key_path();

        tracing::debug!(
            target: TRACING_TARGET_AUTH_KEYS,
            path = %path.display(),
            "loading encoding key from file",
        );

        let pem_data = tokio::fs::read(path).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_AUTH_KEYS,
                path = %path.display(),
                error = %e,
                "failed to read encoding key file",
            );

            ServiceError::file_system("failed to read encoding key file").with_source(e)
        })?;

        let key = EncodingKey::from_ed_pem(&pem_data).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_AUTH_KEYS,
                path = %path.display(),
                error = %e,
                "failed to parse encoding key PEM data",
            );

            ServiceError::auth("invalid encoding key PEM format").with_source(e)
        })?;

        tracing::debug!(
            target: TRACING_TARGET_AUTH_KEYS,
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
MC4CAQAwBQYDK2VwBCIEIJ+DYvh6SEqVTm50DFtMDoQikTmiCqirVv9mWG9qfSnF
-----END PRIVATE KEY-----"#;

    const TEST_PUBLIC_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAGb9ECWmEzf6FQbrBZ9w7lshQhqowtrbLDFw4rXAxZuE=
-----END PUBLIC KEY-----"#;

    #[tokio::test]
    async fn load_valid_keys() {
        let temp_dir = TempDir::new().unwrap();
        let pub_path = temp_dir.path().join("public.pem");
        let priv_path = temp_dir.path().join("private.pem");

        fs::write(&pub_path, TEST_PUBLIC_KEY).unwrap();
        fs::write(&priv_path, TEST_PRIVATE_KEY).unwrap();

        let keys = SessionKeys::new(&pub_path, &priv_path).await.unwrap();
        assert!(keys.validate_keys().is_ok());
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
    async fn reject_mismatched_keys() {
        let temp_dir = TempDir::new().unwrap();
        let pub_path = temp_dir.path().join("public.pem");
        let wrong_priv_path = temp_dir.path().join("wrong_private.pem");

        fs::write(&pub_path, TEST_PUBLIC_KEY).unwrap();
        fs::write(
            &wrong_priv_path,
            r#"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIFhQrCxTwEJ4aYZp4QWc5jDjQw3gGkwLG6D8FP+CvKgA
-----END PRIVATE KEY-----"#,
        )
        .unwrap();

        let keys = SessionKeys::new(&pub_path, &wrong_priv_path).await.unwrap();
        assert!(keys.validate_keys().is_err());
    }
}
