//! MySQL provider.

mod config;
mod input;
mod output;

pub use config::MysqlConfig;

use opendal::{Operator, services};

use crate::error::{Error, Result};

/// MySQL provider for relational data.
#[derive(Clone)]
pub struct MysqlProvider {
    operator: Operator,
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

        Ok(Self { operator })
    }
}

impl std::fmt::Debug for MysqlProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MysqlProvider").finish()
    }
}
