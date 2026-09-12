//! Shared infrastructure clients.
//!
//! [`Infra`] bundles the ambient external client handles—Postgres, NATS, and the
//! blob store—that nearly every service and worker needs. They are not domain
//! collaborators but the surrounding infrastructure, so grouping them keeps
//! constructors from re-listing the same set and shrinks the wiring in
//! [`ServiceState::from_config`]. [`Infra::from_config`] connects all three from
//! their configs, so it owns the fail-fast startup checks. Every field is an
//! `Arc`-backed handle, so cloning `Infra` is cheap.
//!
//! [`ServiceState::from_config`]: crate::service::ServiceState::from_config

use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use nvisy_s3::{BlobStore, S3Config};

use crate::{Error, Result};

/// The ambient external client handles shared across services and workers.
#[derive(Clone)]
pub struct Infra {
    /// The Postgres client (connection pool handle).
    pub postgres: PgClient,
    /// The NATS client (JetStream, KV, messaging).
    pub nats: NatsClient,
    /// The S3-compatible blob store (files, audits, avatars).
    pub blobs: BlobStore,
}

impl Infra {
    /// Connects every ambient client from its config and bundles them into a
    /// single shared handle.
    ///
    /// Each connection fails fast at startup: Postgres applies pending migrations,
    /// and the blob store is pinged so a bad endpoint, wrong credentials, or a
    /// missing bucket surfaces here rather than at the first upload.
    pub async fn from_config(
        postgres_config: PgConfig,
        nats_config: NatsConfig,
        s3_config: S3Config,
    ) -> Result<Self> {
        let postgres = connect_postgres(postgres_config).await?;
        let nats = connect_nats(nats_config).await?;
        let blobs = connect_blobs(s3_config).await?;

        Ok(Self {
            postgres,
            nats,
            blobs,
        })
    }
}

/// Connects to Postgres and applies pending migrations.
async fn connect_postgres(config: PgConfig) -> Result<PgClient> {
    let pg_client = PgClient::new(config).map_err(|e| {
        Error::external("postgres", "Failed to create database client").with_source(e)
    })?;

    pg_client.run_pending_migrations().await.map_err(|e| {
        Error::external("postgres", "Failed to apply database migrations").with_source(e)
    })?;

    Ok(pg_client)
}

/// Connects to the NATS server.
async fn connect_nats(config: NatsConfig) -> Result<NatsClient> {
    NatsClient::connect(config)
        .await
        .map_err(|e| Error::external("NATS", "Failed to connect to NATS").with_source(e))
}

/// Connects to the S3-compatible blob store and verifies it is reachable.
///
/// The AWS SDK builds its client lazily, so [`BlobStore::connect`] alone never
/// touches the network. A follow-up [`ping`](BlobStore::ping) makes a bad
/// endpoint, wrong credentials, or missing bucket fail at startup — matching the
/// fail-fast contract of the Postgres and NATS connectors — rather than at the
/// first upload.
async fn connect_blobs(config: S3Config) -> Result<BlobStore> {
    let blobs = BlobStore::connect(&config)
        .await
        .map_err(|e| Error::external("S3", "Failed to connect to the blob store").with_source(e))?;

    blobs.ping().await.map_err(|e| {
        Error::external("S3", "Blob store is unreachable or its bucket is missing").with_source(e)
    })?;

    Ok(blobs)
}
