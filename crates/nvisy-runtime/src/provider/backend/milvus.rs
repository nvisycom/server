//! Milvus vector database provider.

use nvisy_dal::provider::MilvusConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Default Milvus port.
fn default_milvus_port() -> u16 {
    19530
}

/// Milvus credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilvusCredentials {
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
}

/// Milvus parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilvusParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Collection name.
    pub collection: String,
    /// Database name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

impl MilvusParams {
    /// Combines params with credentials to create a full provider config.
    pub fn into_config(self, credentials: MilvusCredentials) -> MilvusConfig {
        let mut config = MilvusConfig::new(credentials.host)
            .with_port(credentials.port)
            .with_collection(self.collection);

        if let Some((username, password)) = credentials.username.zip(credentials.password) {
            config = config.with_credentials(username, password);
        }
        if let Some(database) = self.database {
            config = config.with_database(database);
        }
        if let Some(dimensions) = self.dimensions {
            config = config.with_dimensions(dimensions);
        }

        config
    }
}
