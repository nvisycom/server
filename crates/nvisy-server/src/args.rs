//! Aggregated service configuration, shared by the first-party CLI and any
//! downstream binary that embeds this server.
//!
//! [`ServiceArgs`] flattens every config [`ServiceState`] needs into one
//! `clap::Args` group, and [`ServiceState::from_args`] turns it into a running
//! state. A wrapping binary can `#[clap(flatten)]` this struct instead of
//! re-declaring the individual config types.
//!
//! The webhook client is deliberately *not* part of the aggregate: it is a
//! pluggable [`WebhookService`], passed to [`from_args`](ServiceState::from_args)
//! so a caller can choose its own implementation.

use nvisy_nats::NatsConfig;
use nvisy_postgres::PgConfig;
use nvisy_webhook::WebhookService;

use crate::Result;
use crate::middleware::UploadConfig;
use crate::service::{
    CryptoConfig, EngineConfig, HealthConfig, S3Config, ServiceState, SessionKeysConfig, SyncConfig,
};

/// Tracing target for configuration echoes emitted by the config aggregates.
pub(crate) const TRACING_TARGET_CONFIG: &str = "nvisy_server::config";

/// Every external-service and resource config [`ServiceState`] is built from.
///
/// Grouped so a binary flattens one struct rather than the ten configs it wraps;
/// [`ServiceState::from_args`] consumes it. The `clap::Args` derive is gated on
/// the `cli` feature.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
#[must_use = "config does nothing unless you use it"]
pub struct ServiceArgs {
    /// Postgres database configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub postgres: PgConfig,

    /// NATS configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub nats: NatsConfig,

    /// S3-compatible blob storage configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub s3: S3Config,

    /// JWT session key paths.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub session_keys: SessionKeysConfig,

    /// Master encryption key path.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub crypto: CryptoConfig,

    /// Redaction engine configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub engine: EngineConfig,

    /// Health monitoring configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub health: HealthConfig,

    /// Connection sync configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub sync: SyncConfig,

    /// Request body size limits (server-wide hard caps).
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub upload: UploadConfig,
}

impl ServiceArgs {
    /// Logs the service configuration at info level (no secrets).
    ///
    /// A binary calls this to echo its resolved service settings at startup; a
    /// downstream that flattens `ServiceArgs` gets the same echo without
    /// re-implementing it.
    pub fn log(&self) {
        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            max_body_bytes = self.upload.max_body_bytes,
            max_file_body_bytes = self.upload.max_file_body_bytes,
            "Upload configuration"
        );

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            postgres_max_connections = self.postgres.postgres_max_connections,
            postgres_connection_timeout = ?self.postgres.postgres_connection_timeout,
            postgres_idle_timeout = ?self.postgres.postgres_idle_timeout,
            "Database configuration"
        );
    }
}

impl ServiceState {
    /// Initializes application state from an aggregated [`ServiceArgs`] and a
    /// caller-provided [`WebhookService`].
    ///
    /// Centralizes the full config-to-state wiring so a binary need not repeat
    /// it. The webhook client is injected rather than derived from config, so a
    /// caller can supply any implementation (the first-party CLI uses the
    /// reqwest-based one).
    pub async fn from_args(args: ServiceArgs, webhook: WebhookService) -> Result<Self> {
        Self::from_config(
            args.postgres,
            args.nats,
            args.session_keys,
            args.crypto,
            args.engine,
            args.health,
            args.sync,
            webhook,
            args.upload,
            args.s3,
        )
        .await
    }
}

/// Compile-time guard for the downstream-embedding contract: a wrapping state
/// `S` that embeds [`ServiceState`] (via [`FromRef`]) can carry its own
/// `ApiRouter<S>` routes through [`CustomRoutes<S>`] and compose them with the
/// built-ins via [`routes`](crate::routes). Never executed; it exists so this
/// contract cannot regress silently.
#[cfg(test)]
mod embed_contract {
    use aide::axum::ApiRouter;
    use axum::extract::FromRef;

    use crate::handler::{CustomRoutes, routes};
    use crate::service::ServiceState;

    #[derive(Clone)]
    struct CloudServiceState {
        inner: ServiceState,
    }

    impl FromRef<CloudServiceState> for ServiceState {
        fn from_ref(state: &CloudServiceState) -> Self {
            state.inner.clone()
        }
    }

    #[allow(dead_code)]
    fn composes(cloud: CloudServiceState) -> ApiRouter<CloudServiceState> {
        let cloud_route = ApiRouter::<CloudServiceState>::new();
        let custom = CustomRoutes::<CloudServiceState>::new().add_private_routes(cloud_route);
        routes(custom, cloud)
    }
}
