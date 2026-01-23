//! PostgreSQL provider.

mod config;
mod input;
mod output;

pub use config::{PostgresCredentials, PostgresParams};
use opendal::{Operator, services};

use crate::core::IntoProvider;
use crate::error::Error;

/// PostgreSQL provider for relational data.
#[derive(Clone)]
pub struct PostgresProvider {
    operator: Operator,
}

#[async_trait::async_trait]
impl IntoProvider for PostgresProvider {
    type Credentials = PostgresCredentials;
    type Params = PostgresParams;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let mut builder = services::Postgresql::default()
            .connection_string(&credentials.connection_string)
            .table(&params.table);

        if let Some(ref schema) = params.schema {
            builder = builder.root(schema);
        }

        let operator = Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| Error::connection(e.to_string()))?;

        Ok(Self { operator })
    }
}

impl std::fmt::Debug for PostgresProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresProvider").finish()
    }
}
