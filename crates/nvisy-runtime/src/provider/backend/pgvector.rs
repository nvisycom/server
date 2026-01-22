//! pgvector (PostgreSQL extension) provider.

use nvisy_dal::provider::PgVectorConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::WorkflowResult;

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

impl IntoProvider for PgVectorParams {
    type Credentials = PgVectorCredentials;
    type Output = PgVectorConfig;

    fn into_provider(self, credentials: Self::Credentials) -> WorkflowResult<Self::Output> {
        Ok(PgVectorConfig::new(credentials.connection_url, self.dimensions).with_table(self.table))
    }
}
