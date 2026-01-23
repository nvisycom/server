//! pgvector DataOutput implementation.

use async_trait::async_trait;
use diesel::sql_types::Text;
use diesel_async::RunQueryDsl;

use super::PgVectorProvider;
use crate::core::DataOutput;
use crate::datatype::Embedding;
use crate::error::{Error, Result};

#[async_trait]
impl DataOutput for PgVectorProvider {
    type Item = Embedding;

    async fn write(&self, items: Vec<Embedding>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let table = self.table();

        let dimensions = <[_]>::first(&items)
            .map(|v| v.vector.len())
            .ok_or_else(|| Error::invalid_input("No embeddings provided"))?;

        self.ensure_collection(table, dimensions).await?;

        let mut conn = self.get_conn().await?;

        for v in items {
            let vector_str = format!(
                "[{}]",
                v.vector
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let metadata_json =
                serde_json::to_string(&v.metadata).unwrap_or_else(|_| "{}".to_string());

            let upsert_query = format!(
                r#"
                INSERT INTO {} (id, vector, metadata)
                VALUES ($1, $2::vector, $3::jsonb)
                ON CONFLICT (id) DO UPDATE SET
                    vector = EXCLUDED.vector,
                    metadata = EXCLUDED.metadata
                "#,
                table
            );

            diesel::sql_query(&upsert_query)
                .bind::<Text, _>(&v.id)
                .bind::<Text, _>(&vector_str)
                .bind::<Text, _>(&metadata_json)
                .execute(&mut conn)
                .await
                .map_err(|e| Error::provider(e.to_string()))?;
        }

        Ok(())
    }
}
