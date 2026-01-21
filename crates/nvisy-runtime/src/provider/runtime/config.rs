//! Runtime configuration.

use serde::{Deserialize, Serialize};

/// Default maximum file size: 100 MB.
const DEFAULT_MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Configuration for the runtime service with sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Maximum file size in bytes (optional).
    pub max_file_size: Option<u64>,
}

impl RuntimeConfig {
    /// Creates a new runtime configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_file_size: None,
        }
    }

    /// Returns the maximum file size, using the default if not set.
    #[inline]
    #[must_use]
    pub fn max_file_size(&self) -> u64 {
        self.max_file_size.unwrap_or(DEFAULT_MAX_FILE_SIZE)
    }

    /// Set the maximum file size in bytes.
    #[must_use]
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = Some(size);
        self
    }

    /// Validate the configuration and return any issues.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_file_size == Some(0) {
            return Err("Maximum file size cannot be zero".to_string());
        }
        Ok(())
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_config() {
        let config = RuntimeConfig::new();
        assert_eq!(config.max_file_size(), DEFAULT_MAX_FILE_SIZE);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_builder() {
        let config = RuntimeConfig::new().with_max_file_size(50 * 1024 * 1024);
        assert_eq!(config.max_file_size(), 50 * 1024 * 1024);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let valid_config = RuntimeConfig::new();
        assert!(valid_config.validate().is_ok());

        let zero_size = RuntimeConfig::new().with_max_file_size(0);
        assert!(zero_size.validate().is_err());
    }
}
