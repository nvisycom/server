//! PostgreSQL provider.
//!
//! Provides relational data operations using a connection pool.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::core::{
    DataInput, DataOutput, InputStream, Provider, Record, RelationalContext, RelationalParams,
};
use crate::python::{self, PyDataInput, PyDataOutput, PyProvider};

/// Credentials for PostgreSQL connection.
///
/// Uses a connection string (DSN) format: `postgres://user:pass@host:port/database`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresCredentials {
    /// PostgreSQL connection string (DSN).
    pub dsn: String,
}

/// Parameters for PostgreSQL operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresParams {
    /// Schema name (defaults to "public").
    #[serde(default = "default_schema")]
    pub schema: String,
    /// Relational parameters (table, pagination).
    #[serde(flatten)]
    pub relational: RelationalParams,
}

fn default_schema() -> String {
    "public".to_string()
}

/// PostgreSQL provider for relational data operations.
pub struct PostgresProvider {
    inner: PyProvider,
    input: PyDataInput<Record, RelationalContext>,
    output: PyDataOutput<Record>,
}

#[async_trait::async_trait]
impl Provider for PostgresProvider {
    type Credentials = PostgresCredentials;
    type Params = PostgresParams;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = python::connect("postgres", credentials, params).await?;
        Ok(Self {
            input: inner.as_data_input(),
            output: inner.as_data_output(),
            inner,
        })
    }

    async fn disconnect(self) -> nvisy_core::Result<()> {
        self.inner.disconnect().await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl DataInput for PostgresProvider {
    type Context = RelationalContext;
    type Item = Record;

    async fn read(&self, ctx: &Self::Context) -> Result<InputStream<Self::Item>> {
        self.input.read(ctx).await
    }
}

#[async_trait::async_trait]
impl DataOutput for PostgresProvider {
    type Item = Record;

    async fn write(&self, items: Vec<Self::Item>) -> Result<()> {
        self.output.write(items).await
    }
}

impl std::fmt::Debug for PostgresProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresProvider").finish_non_exhaustive()
    }
}
