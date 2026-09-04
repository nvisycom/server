//! Shared infrastructure clients.
//!
//! [`Infra`] bundles the ambient clients—Postgres, NATS, the crypto service, and
//! the blob store—that nearly every service and worker needs. They are not domain
//! collaborators but the surrounding infrastructure, so grouping them keeps
//! constructors from re-listing the same set and shrinks the wiring in
//! [`ServiceState::from_config`]. Every field is an `Arc`-backed handle, so
//! cloning `Infra` is cheap.
//!
//! [`ServiceState::from_config`]: crate::service::ServiceState::from_config

use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
use nvisy_s3::BlobStore;

use crate::service::CryptoService;

/// The ambient infrastructure clients shared across services and workers.
#[derive(Clone)]
pub struct Infra {
    /// The Postgres client (connection pool handle).
    pub postgres: PgClient,
    /// The NATS client (JetStream, KV, messaging).
    pub nats: NatsClient,
    /// The encryption service (master key + per-workspace derivation).
    pub crypto: CryptoService,
    /// The S3-compatible blob store (files, audits, avatars).
    pub blobs: BlobStore,
}

impl Infra {
    /// Bundles the ambient clients into a single shared handle.
    pub fn new(
        postgres: PgClient,
        nats: NatsClient,
        crypto: CryptoService,
        blobs: BlobStore,
    ) -> Self {
        Self {
            postgres,
            nats,
            crypto,
            blobs,
        }
    }
}
