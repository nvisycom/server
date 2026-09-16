//! Workspace purge configuration.

use std::time::Duration;

/// Default workspace-purge grace window (30 days): how long a soft-deleted
/// workspace stays recoverable before it is torn down.
pub const DEFAULT_PURGE_GRACE: Duration = Duration::from_hours(24 * 30);

/// Workspace purge configuration.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
#[must_use = "config does nothing unless you use it"]
pub struct PurgeConfig {
    /// Grace window after a workspace is soft-deleted before it is torn down. The
    /// workspace stays recoverable for this long; once it elapses its references
    /// are released (its blobs then reclaim on retention) and its rows removed.
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "workspace-purge-grace",
            env = "WORKSPACE_PURGE_GRACE",
            default_value = "30d",
            value_parser = humantime::parse_duration,
        )
    )]
    pub grace: Duration,
}

impl Default for PurgeConfig {
    fn default() -> Self {
        Self {
            grace: DEFAULT_PURGE_GRACE,
        }
    }
}
