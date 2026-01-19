//! Milvus configuration.

use serde::{Deserialize, Serialize};

/// Milvus configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilvusConfig {
    /// Milvus server host.
    pub host: String,
    /// Milvus server port.
    #[serde(default = "default_milvus_port")]
    pub port: u16,
    /// Username for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Password for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Database name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Default collection name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

impl MilvusConfig {
    /// Creates a new Milvus configuration.
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: default_milvus_port(),
            username: None,
            password: None,
            database: None,
            collection: None,
            dimensions: None,
        }
    }

    /// Sets the port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets the credentials.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Sets the database name.
    pub fn with_database(mut self, database: impl Into<String>) -> Self {
        self.database = Some(database.into());
        self
    }

    /// Sets the default collection.
    pub fn with_collection(mut self, collection: impl Into<String>) -> Self {
        self.collection = Some(collection.into());
        self
    }

    /// Sets the vector dimensions.
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}

fn default_milvus_port() -> u16 {
    19530
}
