//! Shared infrastructure clients.
//!
//! [`Infra`] bundles the three ambient clients—Postgres, NATS, and the crypto
//! service—that nearly every service and worker needs. They are not domain
//! collaborators but the surrounding infrastructure, so grouping them keeps
//! constructors from re-listing the same trio and shrinks the wiring in
//! [`ServiceState::from_config`]. Every field is an `Arc`-backed handle, so
//! cloning `Infra` is cheap.
//!
//! [`ServiceState::from_config`]: crate::service::ServiceState::from_config

use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;

use crate::service::CryptoService;

/// The ambient infrastructure clients shared across services and workers.
#[derive(Clone)]
pub struct Infra {
    /// The Postgres client (connection pool handle).
    pub postgres: PgClient,
    /// The NATS client (JetStream, KV, object storage).
    pub nats: NatsClient,
    /// The encryption service (master key + per-workspace derivation).
    pub crypto: CryptoService,
}

impl Infra {
    /// Bundles the ambient clients into a single shared handle.
    pub fn new(postgres: PgClient, nats: NatsClient, crypto: CryptoService) -> Self {
        Self {
            postgres,
            nats,
            crypto,
        }
    }
}
