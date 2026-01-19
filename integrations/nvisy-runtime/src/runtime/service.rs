//! Runtime service for document processing.

use derive_more::{Deref, DerefMut};
use nvisy_rt_engine::{Engine, EngineConfig};

use super::RuntimeConfig;

/// Runtime service for document processing.
///
/// Wraps the nvisy runtime engine and provides document loading
/// and processing capabilities for workflows.
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
