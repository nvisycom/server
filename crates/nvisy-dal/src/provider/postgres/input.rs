//! PostgreSQL DataInput implementation.

use std::collections::HashMap;

use async_trait::async_trait;
use futures::StreamExt;

use super::PostgresProvider;
use crate::core::{DataInput, InputStream, RelationalContext};
use crate::datatype::Record;
use crate::error::{Error, Result};

#[async_trait]
impl DataInput for PostgresProvider {
    type Item = Record;
    type Context = RelationalContext;

    async fn read(&self, ctx: &RelationalContext) -> Result<InputStream<Record>> {
        let prefix = ctx.table.as_deref().unwrap_or("");
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
