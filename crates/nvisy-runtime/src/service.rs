//! Runtime service for document processing.

use derive_more::{Deref, DerefMut};
use nvisy_rt_engine::{Engine, EngineConfig};
use serde::{Deserialize, Serialize};

#[cfg(feature = "config")]
use clap::Args;

/// Configuration for the runtime service with sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct RuntimeConfig {
    /// Maximum file size in bytes (optional).
    #[cfg_attr(
        feature = "config",
        arg(long = "runtime-max-file-size", env = "RUNTIME_MAX_FILE_SIZE")
    )]
    pub runtime_max_file_size: Option<u64>,
}

// Default values
const DEFAULT_MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

impl RuntimeConfig {
    /// Creates a new runtime configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runtime_max_file_size: None,
        }
    }

    /// Returns the maximum file size, using the default if not set.
    #[inline]
    #[must_use]
    pub fn max_file_size(&self) -> u64 {
        self.runtime_max_file_size.unwrap_or(DEFAULT_MAX_FILE_SIZE)
    }

    /// Set the maximum file size in bytes.
    #[must_use]
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.runtime_max_file_size = Some(size);
        self
    }

    /// Validate the configuration and return any issues.
    pub fn validate(&self) -> Result<(), String> {
        if self.runtime_max_file_size == Some(0) {
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

/// Runtime service for document processing.
///
/// Wraps the nvisy runtime engine and provides document loading
/// and processing capabilities for the server.
///
/// This service derefs to the underlying [`Engine`], allowing direct
/// access to all engine methods.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct RuntimeService {
    #[deref]
    #[deref_mut]
    engine: Engine,
}

impl RuntimeService {
    /// Creates a new runtime service with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
        }
    }

    /// Creates a new runtime service with custom configuration.
    #[must_use]
    pub fn with_config(config: &RuntimeConfig) -> Self {
        let engine_config = EngineConfig {
            max_file_size: Some(config.max_file_size()),
            ..Default::default()
        };
        Self {
            engine: Engine::with_config(engine_config),
        }
    }

    /// Returns a reference to the underlying engine.
    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Returns a mutable reference to the underlying engine.
    #[must_use]
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }
}

impl Default for RuntimeService {
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

    #[test]
    fn test_service_deref() {
        let service = RuntimeService::new();
        // Test that we can call Engine methods directly via Deref
        let _extensions = service.supported_extensions();
        let _mimes = service.supported_mime_types();
    }

    #[test]
    fn test_service_with_config() {
        let config = RuntimeConfig::new().with_max_file_size(10 * 1024 * 1024);
        let _service = RuntimeService::with_config(&config);
    }
}
