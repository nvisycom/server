//! Adapter that lets the `oauth2` crate make its token requests through a shared
//! [`reqwest::Client`], with retry and tracing middleware layered on.
//!
//! `oauth2`'s built-in `reqwest` integration targets a different major version of
//! `reqwest` than the workspace uses, so its `Client` type does not match ours.
//! [`OAuthHttpClient`] implements `oauth2`'s [`AsyncHttpClient`] over our shared
//! client, translating between the `http` crate's request/response types (which
//! `oauth2` speaks) and `reqwest`. Token requests are short RPC calls, so
//! transient failures are retried with exponential backoff.

use std::pin::Pin;

use oauth2::{AsyncHttpClient, HttpRequest, HttpResponse};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_tracing::TracingMiddleware;

/// Number of times a transient token-request failure is retried before giving up.
const MAX_RETRIES: u32 = 3;

/// An error from the OAuth HTTP transport.
#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    /// The middleware chain or the underlying reqwest call failed.
    #[error("oauth http request failed: {0}")]
    Request(#[from] reqwest_middleware::Error),
    /// The response could not be rebuilt as an `http::Response`.
    #[error("oauth http response was invalid: {0}")]
    Response(#[from] http::Error),
}

/// An `oauth2` async HTTP client backed by a shared [`reqwest::Client`] with
/// retry and tracing middleware.
///
/// A `reqwest::Client` is an `Arc` handle, so building the middleware client over
/// one is cheap and shares its connection pool and TLS/proxy configuration.
pub struct OAuthHttpClient {
    http: ClientWithMiddleware,
}

impl OAuthHttpClient {
    /// Wraps a shared reqwest client with retry (exponential backoff) and
    /// tracing middleware for OAuth token requests.
    pub fn new(client: reqwest::Client) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(MAX_RETRIES);
        let http = ClientBuilder::new(client)
            .with(TracingMiddleware::default())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        Self { http }
    }

    /// Sends one `oauth2` request through the middleware client and returns the
    /// reply, rebuilt as the `http::Response` `oauth2` expects.
    ///
    /// Mirrors `oauth2`'s own `reqwest::Client` integration (which targets an
    /// incompatible `reqwest` version): convert the request with `try_into`, then
    /// copy status, HTTP version, and headers onto the rebuilt response.
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, HttpClientError> {
        let request =
            reqwest::Request::try_from(request).map_err(reqwest_middleware::Error::from)?;
        let response = self.http.execute(request).await?;

        let mut builder = http::Response::builder()
            .status(response.status())
            .version(response.version());
        for (name, value) in response.headers() {
            builder = builder.header(name, value);
        }
        let body = response
            .bytes()
            .await
            .map_err(reqwest_middleware::Error::from)?
            .to_vec();
        Ok(builder.body(body)?)
    }
}

impl<'c> AsyncHttpClient<'c> for OAuthHttpClient {
    type Error = HttpClientError;
    type Future = Pin<Box<dyn Future<Output = Result<HttpResponse, Self::Error>> + Send + 'c>>;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(self.send(request))
    }
}
