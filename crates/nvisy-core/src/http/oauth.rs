//! An `oauth2` transport over the shared HTTP stack.

use std::pin::Pin;

use oauth2::{AsyncHttpClient, HttpRequest, HttpResponse};
use reqwest_middleware::ClientWithMiddleware;

use super::{ExponentialBackoff, middleware_client};

/// Number of times a transient token-request failure is retried.
const MAX_RETRIES: u32 = 3;

/// An error from the OAuth HTTP transport.
#[derive(Debug, thiserror::Error)]
pub enum OAuthHttpError {
    /// The middleware chain or the underlying reqwest call failed.
    #[error("oauth http request failed: {0}")]
    Request(#[from] reqwest_middleware::Error),
    /// The response could not be rebuilt as an `http::Response`.
    #[error("oauth http response was invalid: {0}")]
    Response(#[from] http::Error),
}

/// An `oauth2` (and so `openidconnect`) async HTTP client that sends requests
/// through [`middleware_client`], retrying transient failures.
///
/// `oauth2`'s built-in reqwest integration targets another reqwest major
/// version, so this implements its [`AsyncHttpClient`] over the workspace's
/// client, translating between the `http` crate's request and response types
/// and reqwest's. Build it once and share it: cloning is cheap.
#[derive(Clone)]
pub struct OAuthHttpClient {
    http: ClientWithMiddleware,
}

impl OAuthHttpClient {
    /// Wraps `client` for OAuth requests, retrying transient failures with
    /// exponential backoff.
    #[must_use]
    pub fn new(client: reqwest::Client) -> Self {
        let retry = ExponentialBackoff::builder().build_with_max_retries(MAX_RETRIES);
        Self {
            http: middleware_client(client, retry),
        }
    }

    /// Sends one `oauth2` request and returns the reply rebuilt as the
    /// `http::Response` `oauth2` expects: status, HTTP version, headers, body.
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, OAuthHttpError> {
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
    type Error = OAuthHttpError;
    type Future = Pin<Box<dyn Future<Output = Result<HttpResponse, Self::Error>> + Send + 'c>>;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(self.send(request))
    }
}
