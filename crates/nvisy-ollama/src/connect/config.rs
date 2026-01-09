//! Ollama client configuration.

#[cfg(feature = "config")]
use clap::Args;
use serde::{Deserialize, Serialize};

/// Configuration for the Ollama client.
///
/// This configuration is used to connect to an Ollama server.
/// The `ollama-rs` client uses host and port internally.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct OllamaConfig {
    /// Ollama server host (e.g., "localhost" or "192.168.1.100")
    #[cfg_attr(
        feature = "config",
        arg(long = "ollama-host", env = "OLLAMA_HOST", default_value = "localhost")
    )]
    #[serde(default = "default_host")]
    pub ollama_host: String,

    /// Ollama server port
    #[cfg_attr(
        feature = "config",
        arg(long = "ollama-port", env = "OLLAMA_PORT", default_value = "11434")
    )]
    #[serde(default = "default_port")]
    pub ollama_port: u16,

    /// Default model for embeddings (e.g., "nomic-embed-text")
    #[cfg_attr(
        feature = "config",
        arg(long = "ollama-embedding-model", env = "OLLAMA_EMBEDDING_MODEL")
    )]
    pub embedding_model: Option<String>,

    /// Default model for VLM/chat (e.g., "llava", "llama3")
    #[cfg_attr(
        feature = "config",
        arg(long = "ollama-vlm-model", env = "OLLAMA_VLM_MODEL")
    )]
    pub vlm_model: Option<String>,
}

fn default_host() -> String {
    "localhost".to_string()
}

fn default_port() -> u16 {
    11434
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            ollama_host: default_host(),
            ollama_port: default_port(),
            embedding_model: None,
            vlm_model: None,
        }
    }
}

impl OllamaConfig {
    /// Create a new configuration with host and port.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            ollama_host: host.into(),
            ollama_port: port,
            embedding_model: None,
            vlm_model: None,
        }
    }

    /// Returns the full URL for the Ollama server.
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.ollama_host, self.ollama_port)
    }

    /// Set the host.
    #[must_use]
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.ollama_host = host.into();
        self
    }

    /// Set the port.
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.ollama_port = port;
        self
    }

    /// Set the default embedding model.
    #[must_use]
    pub fn with_embedding_model(mut self, model: impl Into<String>) -> Self {
        self.embedding_model = Some(model.into());
        self
    }

    /// Set the default VLM model.
    #[must_use]
    pub fn with_vlm_model(mut self, model: impl Into<String>) -> Self {
        self.vlm_model = Some(model.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OllamaConfig::default();
        assert_eq!(config.ollama_host, "localhost");
        assert_eq!(config.ollama_port, 11434);
        assert_eq!(config.url(), "http://localhost:11434");
    }

    #[test]
    fn test_new_config() {
        let config = OllamaConfig::new("192.168.1.100", 8080);
        assert_eq!(config.ollama_host, "192.168.1.100");
        assert_eq!(config.ollama_port, 8080);
        assert_eq!(config.url(), "http://192.168.1.100:8080");
    }

    #[test]
    fn test_builder_pattern() {
        let config = OllamaConfig::default()
            .with_host("remote-server")
            .with_port(9999)
            .with_embedding_model("nomic-embed-text")
            .with_vlm_model("llava");

        assert_eq!(config.ollama_host, "remote-server");
        assert_eq!(config.ollama_port, 9999);
        assert_eq!(config.embedding_model, Some("nomic-embed-text".to_string()));
        assert_eq!(config.vlm_model, Some("llava".to_string()));
    }
}
