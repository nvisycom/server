//! The shared outbound HTTP stack.
//!
//! Every client that calls an external service layers the same middleware on a
//! [`reqwest::Client`]: request tracing, then retry of transient failures with
//! exponential backoff. [`middleware_client`] builds that stack; each caller
//! keeps its own base client, since timeouts and redirect policy differ by use
//! (a streamed file transfer, a webhook delivery, a token request).
//! `OAuthHttpClient` runs `oauth2` requests through the same stack.

#[cfg(feature = "oauth2")]
mod oauth;

#[cfg(feature = "oauth2")]
pub use oauth::{OAuthHttpClient, OAuthHttpError};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::RetryTransientMiddleware;
pub use reqwest_retry::policies::ExponentialBackoff;
use reqwest_tracing::TracingMiddleware;

/// Wraps `client` with request tracing and retry of transient failures under
/// `retry`.
///
/// A `reqwest::Client` is an `Arc` handle, so the result shares its connection
/// pool, TLS, and timeout configuration.
#[must_use]
pub fn middleware_client(
    client: reqwest::Client,
    retry: ExponentialBackoff,
) -> ClientWithMiddleware {
    ClientBuilder::new(client)
        .with(TracingMiddleware::default())
        .with(RetryTransientMiddleware::new_with_policy(retry))
        .build()
}
