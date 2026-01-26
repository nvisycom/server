//! PostgreSQL provider.
//!
//! Provides relational data operations using a connection pool.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::core::{
    DataInput, DataOutput, InputStream, Provider, RelationalContext, RelationalParams,
};
use crate::datatype::Record;
use crate::python::{PyDataInput, PyDataOutput, PyProvider, PyProviderLoader};

/// Credentials for PostgreSQL connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresCredentials {
    /// Database host.
    pub host: String,
    /// Database port.
    pub port: u16,
    /// Database user.
    pub user: String,
    /// Database password.
    pub password: String,
    /// Database name.
    pub database: String,
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

impl PostgresProvider {
    /// Disconnects from the database.
    pub async fn disconnect(self) -> Result<()> {
        self.inner.disconnect().await
    }
}

#[async_trait::async_trait]
impl Provider for PostgresProvider {
    type Credentials = PostgresCredentials;
    type Params = PostgresParams;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let loader = PyProviderLoader::new().map_err(crate::Error::from)?;
        let creds_json = serde_json::to_value(&credentials).map_err(crate::Error::from)?;
        let params_json = serde_json::to_value(&params).map_err(crate::Error::from)?;

        let inner = loader
            .load("postgres", creds_json, params_json)
            .await
            .map_err(crate::Error::from)?;
        let input = PyDataInput::new(PyProvider::new(inner.clone_py_object()));
        let output = PyDataOutput::new(PyProvider::new(inner.clone_py_object()));

        Ok(Self {
            inner,
            input,
            output,
        })
    }
}

#[async_trait::async_trait]
impl DataInput for PostgresProvider {
    type Item = Record;
    type Context = RelationalContext;

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
