//! Deployment configuration for the integration subsystem.
//!
//! Connection configs can carry a caller-supplied endpoint URL (an object
//! store's custom `endpoint`, an inference provider's `base_url`), which the
//! server then reaches with the connection's credentials. [`EndpointPolicy`]
//! governs how those endpoints are validated — permissive for self-hosting,
//! strict for a multi-tenant cloud — so the deployment mode decides the SSRF and
//! cleartext-credential posture in one place. It also holds the sync engine's
//! concurrency tunable, so every integration deployment knob lives here.

use nvisy_core::net::EndpointPolicy;

/// Default number of objects imported concurrently per sync. Kept well below the
/// default Postgres pool size (10) since each in-flight import briefly checks out
/// a pooled connection for its bookkeeping, and the pool is shared with the HTTP
/// handlers and other workers.
pub const DEFAULT_IMPORT_CONCURRENCY: usize = 4;

/// Deployment configuration for the integration subsystem: how workspace
/// connections may be created (endpoint policy) and synced (import concurrency).
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
#[must_use = "config does nothing unless you use it"]
pub struct IntegrationConfig {
    /// How caller-supplied connection endpoints are validated.
    ///
    /// `permissive` (self-hosted): https anywhere, plaintext http only for
    /// loopback emulators. `strict` (cloud): https only, and the host must
    /// resolve entirely to globally routable addresses.
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "endpoint-policy",
            env = "CONNECTION_ENDPOINT_POLICY",
            value_enum,
            default_value_t = EndpointPolicy::Permissive,
        )
    )]
    pub endpoint_policy: EndpointPolicy,

    /// Maximum objects imported concurrently within a single sync. Bounds the
    /// in-flight fetch/decrypt/store pipelines against one connection.
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "sync-import-concurrency",
            env = "SYNC_IMPORT_CONCURRENCY",
            default_value_t = DEFAULT_IMPORT_CONCURRENCY,
        )
    )]
    pub import_concurrency: usize,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            endpoint_policy: EndpointPolicy::default(),
            import_concurrency: DEFAULT_IMPORT_CONCURRENCY,
        }
    }
}
