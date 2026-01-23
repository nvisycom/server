//! MySQL provider.

mod config;
mod input;
mod output;

pub use config::{MysqlCredentials, MysqlParams};
use opendal::{Operator, services};

use crate::core::IntoProvider;
use crate::error::Error;

/// MySQL provider for relational data.
#[derive(Clone)]
pub struct MysqlProvider {
    operator: Operator,
}

#[async_trait::async_trait]
impl IntoProvider for MysqlProvider {
    type Credentials = MysqlCredentials;
    type Params = MysqlParams;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let mut builder = services::Mysql::default()
            .connection_string(&credentials.connection_string)
            .table(&params.table);

        if let Some(ref database) = params.database {
            builder = builder.root(database);
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
