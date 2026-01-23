//! PostgreSQL provider.

mod config;
mod input;
mod output;

pub use config::PostgresConfig;

use opendal::{Operator, services};

use crate::error::{Error, Result};

/// PostgreSQL provider for relational data.
#[derive(Clone)]
pub struct PostgresProvider {
    operator: Operator,
}

impl PostgresProvider {
    /// Creates a new PostgreSQL provider.
    pub fn new(config: &PostgresConfig) -> Result<Self> {
        let mut builder =
            services::Postgresql::default().connection_string(&config.connection_string);

        if let Some(ref table) = config.table {
            builder = builder.table(table);
        }

        if let Some(ref root) = config.schema {
            builder = builder.root(root);
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
