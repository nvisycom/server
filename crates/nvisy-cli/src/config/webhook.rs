//! Webhook HTTP client configuration arguments.

use std::time::Duration;

use clap::Args;
use nvisy_webhook::reqwest::ReqwestConfig;

/// Reqwest HTTP client arguments.
#[derive(Debug, Clone, Args)]
pub struct ReqwestArgs {
    /// HTTP request timeout (e.g. `30s`).
    #[arg(
        long = "http-timeout",
        env = "HTTP_TIMEOUT",
        value_parser = humantime::parse_duration,
    )]
    pub http_timeout: Option<Duration>,

    /// User-Agent header to send with requests.
    #[arg(long = "http-user-agent", env = "HTTP_USER_AGENT")]
    pub user_agent: Option<String>,

    /// Maximum number of retry attempts for transient failures.
    #[arg(
        long = "http-max-retries",
        env = "HTTP_MAX_RETRIES",
        default_value = "3"
    )]
    pub max_retries: u32,

    /// Minimum retry interval (e.g. `500ms`).
    #[arg(
        long = "http-min-retry-interval",
        env = "HTTP_MIN_RETRY_INTERVAL",
        default_value = "500ms",
        value_parser = humantime::parse_duration,
    )]
    pub min_retry_interval: Duration,

    /// Maximum retry interval (e.g. `30s`).
    #[arg(
        long = "http-max-retry-interval",
        env = "HTTP_MAX_RETRY_INTERVAL",
        default_value = "30s",
        value_parser = humantime::parse_duration,
    )]
    pub max_retry_interval: Duration,
}

impl From<ReqwestArgs> for ReqwestConfig {
    fn from(args: ReqwestArgs) -> Self {
        Self {
            http_timeout: args.http_timeout,
            user_agent: args.user_agent,
            max_retries: args.max_retries,
            min_retry_interval: args.min_retry_interval,
            max_retry_interval: args.max_retry_interval,
        }
    }
}
