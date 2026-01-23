//! PostgreSQL DataOutput implementation.

use async_trait::async_trait;

use super::PostgresProvider;
use crate::core::DataOutput;
use crate::datatype::Record;
use crate::error::{Error, Result};

#[async_trait]
impl DataOutput for PostgresProvider {
    type Item = Record;

    async fn write(&self, items: Vec<Record>) -> Result<()> {
        for record in items {
            let key = record
                .get("_key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let value =
                serde_json::to_vec(&record.columns).map_err(|e| Error::provider(e.to_string()))?;

            self.operator
                .write(&key, value)
                .await
                .map_err(|e| Error::provider(e.to_string()))?;
        }
        Ok(())
    }
}
