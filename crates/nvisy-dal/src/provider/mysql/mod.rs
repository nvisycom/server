//! MySQL provider via OpenDAL.

mod config;

use std::collections::HashMap;

use async_trait::async_trait;
pub use config::MysqlConfig;
use futures::StreamExt;
use opendal::{Operator, services};

use crate::core::{Context, DataInput, DataOutput, InputStream};
use crate::datatype::Record;
use crate::error::{Error, Result};

/// MySQL provider for relational data.
#[derive(Clone)]
pub struct MysqlProvider {
    operator: Operator,
    #[allow(dead_code)]
    config: MysqlConfig,
}

impl MysqlProvider {
    /// Creates a new MySQL provider.
    pub fn new(config: &MysqlConfig) -> Result<Self> {
        let mut builder = services::Mysql::default().connection_string(&config.connection_string);

        if let Some(ref table) = config.table {
            builder = builder.table(table);
        }

        if let Some(ref root) = config.database {
            builder = builder.root(root);
        }

        let operator = Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| Error::connection(e.to_string()))?;

        Ok(Self {
            operator,
            config: config.clone(),
        })
    }
}

#[async_trait]
impl DataInput<Record> for MysqlProvider {
    async fn read(&self, ctx: &Context) -> Result<InputStream<'static, Record>> {
        let prefix = ctx.target.as_deref().unwrap_or("");
        let limit = ctx.limit.unwrap_or(usize::MAX);

        let lister = self
            .operator
            .lister(prefix)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        let operator = self.operator.clone();

        let stream = lister.take(limit).filter_map(move |entry_result| {
            let op = operator.clone();
            async move {
                match entry_result {
                    Ok(entry) => {
                        let key = entry.path().to_string();
                        match op.read(&key).await {
                            Ok(data) => {
                                // Parse the value as JSON to get columns
                                let value: serde_json::Value =
                                    serde_json::from_slice(&data.to_bytes())
                                        .unwrap_or(serde_json::json!({}));

                                let columns: HashMap<String, serde_json::Value> =
                                    if let serde_json::Value::Object(map) = value {
                                        map.into_iter().collect()
                                    } else {
                                        let mut cols = HashMap::new();
                                        cols.insert("_key".to_string(), serde_json::json!(key));
                                        cols.insert("_value".to_string(), value);
                                        cols
                                    };

                                Some(Ok(Record::from_columns(columns)))
                            }
                            Err(e) => Some(Err(Error::provider(e.to_string()))),
                        }
                    }
                    Err(e) => Some(Err(Error::provider(e.to_string()))),
                }
            }
        });

        Ok(InputStream::new(Box::pin(stream)))
    }
}

#[async_trait]
impl DataOutput<Record> for MysqlProvider {
    async fn write(&self, _ctx: &Context, items: Vec<Record>) -> Result<()> {
        for record in items {
            // Use _key column as the key, or generate one
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

impl std::fmt::Debug for MysqlProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MysqlProvider").finish()
    }
}
