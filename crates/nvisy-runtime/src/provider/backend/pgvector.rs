//! pgvector (PostgreSQL extension) provider.

use nvisy_dal::provider::PgVectorConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// pgvector credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgVectorCredentials {
    /// PostgreSQL connection URL.
    pub connection_url: String,
}

/// pgvector parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PgVectorParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Table name.
    pub table: String,
    /// Vector dimensions.
    pub dimensions: usize,
}

impl PgVectorParams {
    /// Combines params with credentials to create a full provider config.
    pub fn into_config(self, credentials: PgVectorCredentials) -> PgVectorConfig {
        PgVectorConfig::new(credentials.connection_url, self.dimensions).with_table(self.table)
    }
}
